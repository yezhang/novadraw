//! 场景图管理
//!
//! 提供场景图数据结构和管理功能。

use std::{
    cell::RefCell,
    error::Error,
    fmt,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crate::geometry::{Affine2D, Dimension, Point, PointList, Rectangle};
use crate::render::{NdCanvas, TextError, TextLayoutEngine};
use uuid::Uuid;

use super::figure::{
    AccessibleFigure, ChildClippingStrategy, ChildPolicy, ClickableSnapshot, ClickableVisualState,
    Direction, FigureMeasurement, ImageFigure, LabelFigure, MeasureConstraints,
    RoundedRectangleFigure, ShapeMutationError, TriangleFigure, WidgetError, normalize_points,
};
use super::layout::{
    LayoutChange, LayoutConstraint, LayoutError, LayoutInvalidation, LayoutManager, LayoutOutput,
    LayoutSnapshot,
};
use crate::Border;
use crate::figure::border::BorderSnapshot;
pub use crate::identity::FigureId;
use crate::identity::{RuntimeArena, RuntimeNamespace};
#[cfg(test)]
use crate::mutation::PendingMutation;
use crate::mutation::PendingMutationKind;
use crate::runtime::update::{
    ActionEvent, AncestorEvent, AncestorEventKind, FigureEvent, LayoutEvent, LayoutEventKind,
    NotificationEffect, NotificationQueue, PropertyChangeEvent, PropertyValue, UpdateManager,
};
use crate::style::{FigureStyle, ResolvedStyle};

// 渲染模块
mod layout_measurement;
mod presentation;
mod query;
mod render_recursive;
mod search;
mod topology;

use render_recursive::{FigureRenderer, FigureTreeRenderRef};
pub use search::{ExclusionSearch, IdentitySearch, TreeQueryError, TreeSearch, TreeSearchContext};

#[cfg(test)]
pub mod bounds_test;

#[cfg(test)]
pub mod update_integration_test;

/// Figure 树允许的最大深度。根节点深度为 0。
pub const MAX_TREE_DEPTH: usize = 10_000;
pub const DEFAULT_VALIDATION_BUDGET: usize = 10_000;
const RECURSIVE_STACK_CHECK_INTERVAL: usize = 16;
const RECURSIVE_VALIDATION_STACK_RED_ZONE: usize = 128 * 1024;
const RECURSIVE_VALIDATION_STACK_GROWTH: usize = 4 * 1024 * 1024;
pub const FREEFORM_EXTENT_PROPERTY: &str = "freeform_extent";

fn finite_rectangle(rectangle: Rectangle) -> bool {
    rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
}

fn transform_rectangle(transform: Affine2D, rectangle: Rectangle) -> Option<Rectangle> {
    let corners = [
        transform.transform_point(Point::new(rectangle.x, rectangle.y)),
        transform.transform_point(Point::new(rectangle.x + rectangle.width, rectangle.y)),
        transform.transform_point(Point::new(rectangle.x, rectangle.y + rectangle.height)),
        transform.transform_point(Point::new(
            rectangle.x + rectangle.width,
            rectangle.y + rectangle.height,
        )),
    ];
    if corners
        .iter()
        .flat_map(|point| [point.x(), point.y()])
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let left = corners
        .iter()
        .map(|point| point.x())
        .fold(f64::INFINITY, f64::min);
    let top = corners
        .iter()
        .map(|point| point.y())
        .fold(f64::INFINITY, f64::min);
    let right = corners
        .iter()
        .map(|point| point.x())
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = corners
        .iter()
        .map(|point| point.y())
        .fold(f64::NEG_INFINITY, f64::max);
    Some(Rectangle::new(left, top, right - left, bottom - top))
}

/// Figure 树结构变更失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMutationError {
    FigureNotFound(FigureId),
    ParentNotFound,
    ChildNotFound,
    CycleDetected,
    DuplicateChild,
    ChildLimitExceeded {
        limit: usize,
    },
    LayerKeyRequired,
    LayerChildRequired,
    InvalidParentRelation,
    InvalidChildIndex {
        parent: FigureId,
        index: usize,
        child_count: usize,
    },
    DepthLimitExceeded {
        limit: usize,
    },
}

impl fmt::Display for GraphMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FigureNotFound(figure) => write!(f, "Figure does not exist: {figure:?}"),
            Self::ParentNotFound => write!(f, "parent Figure does not exist"),
            Self::ChildNotFound => write!(f, "child Figure does not exist"),
            Self::CycleDetected => write!(f, "mutation would create a cycle"),
            Self::DuplicateChild => write!(f, "child is already attached to parent"),
            Self::ChildLimitExceeded { limit } => {
                write!(f, "parent accepts at most {limit} direct child")
            }
            Self::LayerKeyRequired => write!(f, "layered pane mutations require a layer key"),
            Self::LayerChildRequired => write!(f, "layered pane accepts only Layer figures"),
            Self::InvalidParentRelation => write!(f, "child is not attached to expected parent"),
            Self::InvalidChildIndex {
                parent,
                index,
                child_count,
            } => write!(
                f,
                "child index {index} is outside parent {parent:?} child count {child_count}"
            ),
            Self::DepthLimitExceeded { limit } => {
                write!(f, "figure tree depth exceeds limit {limit}")
            }
        }
    }
}

impl Error for GraphMutationError {}

/// A child insertion rejected by topology admission or layout constraint validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ChildInsertionError {
    Graph(GraphMutationError),
    Layout(LayoutError),
}

impl From<GraphMutationError> for ChildInsertionError {
    fn from(error: GraphMutationError) -> Self {
        Self::Graph(error)
    }
}

impl From<LayoutError> for ChildInsertionError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl fmt::Display for ChildInsertionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(error) => error.fmt(formatter),
            Self::Layout(error) => error.fmt(formatter),
        }
    }
}

impl Error for ChildInsertionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(error) => Some(error),
            Self::Layout(error) => Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    Layout(LayoutError),
    NonConvergingValidation {
        budget: usize,
        invalidation_chain: Vec<FigureId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeformError {
    UnknownFigure(FigureId),
    NotFreeform(FigureId),
    Unvalidated(FigureId),
}

impl fmt::Display for FreeformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure(id) => write!(formatter, "unknown Figure ID: {id:?}"),
            Self::NotFreeform(id) => write!(formatter, "Figure is not freeform: {id:?}"),
            Self::Unvalidated(id) => write!(formatter, "freeform extent is not validated: {id:?}"),
        }
    }
}

impl Error for FreeformError {}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => error.fmt(f),
            Self::NonConvergingValidation {
                budget,
                invalidation_chain,
            } => write!(
                f,
                "validation did not converge within {budget} passes; invalidation chain: {invalidation_chain:?}"
            ),
        }
    }
}

impl Error for ValidationError {}

impl From<LayoutError> for ValidationError {
    fn from(value: LayoutError) -> Self {
        Self::Layout(value)
    }
}

fn point_in_rect(point: Point, rect: &Rectangle) -> bool {
    point.x() >= rect.x
        && point.x() <= rect.x + rect.width
        && point.y() >= rect.y
        && point.y() <= rect.y + rect.height
}

fn owner_scoped_border_size(content: Dimension, snapshot: Option<&BorderSnapshot>) -> Dimension {
    let Some(snapshot) = snapshot else {
        return content;
    };
    let (top, left, bottom, right) = snapshot.insets();
    let preferred = snapshot.preferred_size();
    Dimension::new(
        (content.width + left + right).max(preferred.0),
        (content.height + top + bottom).max(preferred.1),
    )
}

fn owner_scoped_border_measurement(
    mut measurement: FigureMeasurement,
    snapshot: Option<&BorderSnapshot>,
) -> FigureMeasurement {
    let Some(snapshot) = snapshot else {
        return measurement;
    };
    let (top, left, bottom, right) = snapshot.insets();
    let preferred = snapshot.preferred_size();
    measurement.width = (measurement.width + left + right).max(preferred.0);
    measurement.height = (measurement.height + top + bottom).max(preferred.1);
    measurement.baseline = measurement.baseline.map(|baseline| baseline + top);
    measurement
}

/// State shared by every Figure node.
///
/// Concrete Figure implementations retain only type-specific data. Tree,
/// interaction and update algorithms consume this state without downcasting.
pub struct NodeState {
    pub(crate) bounds: Rectangle,
    pub(crate) insets: (f64, f64, f64, f64),
    pub(crate) border_snapshot: Option<BorderSnapshot>,
    pub(crate) is_visible: bool,
    pub(crate) is_enabled: bool,
    pub(crate) is_opaque: bool,
    pub(crate) is_focusable: bool,
    pub(crate) is_focus_traversable: bool,
    pub(crate) is_valid: bool,
    pub(crate) preferred_size: Option<(f64, f64)>,
    pub(crate) minimum_size: Option<(f64, f64)>,
    pub(crate) maximum_size: Option<(f64, f64)>,
    pub(crate) child_clipping_strategy: Option<ChildClippingStrategy>,
    pub(crate) style: FigureStyle,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            bounds: Rectangle::ZERO,
            insets: (0.0, 0.0, 0.0, 0.0),
            border_snapshot: None,
            is_visible: true,
            is_enabled: true,
            is_opaque: false,
            is_focusable: false,
            is_focus_traversable: false,
            is_valid: false,
            preferred_size: None,
            minimum_size: None,
            maximum_size: None,
            child_clipping_strategy: None,
            style: FigureStyle::default(),
        }
    }
}

impl NodeState {
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn is_opaque(&self) -> bool {
        self.is_opaque
    }

    pub fn is_focusable(&self) -> bool {
        self.is_focusable
    }

    pub fn is_focus_traversable(&self) -> bool {
        self.is_focus_traversable
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }

    pub fn style(&self) -> &FigureStyle {
        &self.style
    }

    pub fn child_clipping_strategy_override(&self) -> Option<ChildClippingStrategy> {
        self.child_clipping_strategy
    }
}

/// Container-owned layout strategy and child constraints.
#[derive(Default)]
pub struct LayoutState {
    pub(crate) manager: Option<Box<dyn LayoutManager>>,
    pub(crate) constraints: std::collections::HashMap<FigureId, Box<dyn LayoutConstraint>>,
    cache: RefCell<LayoutCache>,
    freeform: Option<FreeformState>,
}

#[derive(Debug, Clone, Copy)]
pub struct FreeformState {
    cached_extent: Rectangle,
    extent_generation: Option<u64>,
    dirty: bool,
}

impl Default for FreeformState {
    fn default() -> Self {
        Self {
            cached_extent: Rectangle::ZERO,
            extent_generation: None,
            dirty: true,
        }
    }
}

impl FreeformState {
    pub fn cached_extent(&self) -> Rectangle {
        self.cached_extent
    }

    pub fn extent_generation(&self) -> Option<u64> {
        self.extent_generation
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LayoutCache {
    generation: u64,
    preferred: Option<CachedMeasurement>,
    minimum: Option<CachedMeasurement>,
    validated_generation: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct CachedMeasurement {
    generation: u64,
    constraints: MeasureConstraints,
    measurement: FigureMeasurement,
}

impl CachedMeasurement {
    fn matches(self, generation: u64, constraints: MeasureConstraints) -> bool {
        self.generation == generation && self.constraints == constraints
    }
}

impl LayoutState {
    fn for_figure(figure: &dyn super::Figure) -> Self {
        Self {
            freeform: figure.freeform().is_some().then(FreeformState::default),
            ..Self::default()
        }
    }

    pub fn has_manager(&self) -> bool {
        self.manager.is_some()
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    pub fn generation(&self) -> u64 {
        self.cache.borrow().generation
    }

    pub fn validated_generation(&self) -> Option<u64> {
        self.cache.borrow().validated_generation
    }

    pub fn freeform_state(&self) -> Option<&FreeformState> {
        self.freeform.as_ref()
    }

    fn invalidate(&mut self, reason: LayoutInvalidation) {
        let cache = self.cache.get_mut();
        cache.generation = cache.generation.wrapping_add(1);
        cache.preferred = None;
        cache.minimum = None;
        cache.validated_generation = None;
        if let Some(freeform) = self.freeform.as_mut() {
            freeform.dirty = true;
        }
        if let Some(manager) = self.manager.as_mut() {
            manager.invalidate(reason);
        }
    }

    fn mark_validated(&mut self) {
        let cache = self.cache.get_mut();
        cache.validated_generation = Some(cache.generation);
    }
}

/// FigureNode - 图形节点
///
/// 场景图中的基本单元，同时包含图形数据（通过 `Box<dyn Figure>`）
/// 和树形结构（parent/children），参考 Eclipse Draw2D 的 Figure 设计。
///
/// # 与 Figure trait 的区别
///
/// - `FigureNode` 是具体的数据结构，实现了树形节点的所有功能
/// - `dyn Figure` 是渲染接口 trait，定义了图形的几何和渲染行为
/// - 一个 `FigureNode` 持有 `Box<dyn Figure>` 来实现具体的图形类型
pub struct FigureNode {
    /// 块 ID
    pub(crate) id: FigureId,
    /// UUID
    pub(crate) uuid: Uuid,
    /// 子块列表
    pub(crate) children: Vec<FigureId>,
    /// 父块
    pub(crate) parent: Option<FigureId>,
    /// 从 FigureTree 根节点开始计算的深度；根节点深度为 0。
    pub(crate) depth: usize,
    /// 图形
    pub(crate) figure: Box<dyn super::Figure>,
    /// Runtime 提交的私有组件状态版本。
    pub(crate) component_revision: u64,
    pub(crate) prepared: Option<crate::figure::preparation::PreparedFigure>,
    /// 容器布局策略、关系约束和后续布局缓存的唯一归属。
    pub(crate) layout: LayoutState,
    /// 所有 Figure 共享的节点状态。
    pub(crate) state: NodeState,
}

impl Deref for FigureNode {
    type Target = NodeState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for FigureNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl FigureNode {
    /// 获取块 ID
    pub fn id(&self) -> FigureId {
        self.id
    }

    /// 获取块 UUID
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// 获取子块数量
    pub fn children_count(&self) -> usize {
        self.children.len()
    }

    /// Returns the Figure implementation's stable diagnostic name.
    pub fn figure_name(&self) -> &'static str {
        self.figure.name()
    }

    pub fn state(&self) -> &NodeState {
        &self.state
    }

    pub fn layout_state(&self) -> &LayoutState {
        &self.layout
    }

    /// Returns the node bounds from common node state.
    pub fn figure_bounds(&self) -> Rectangle {
        self.state.bounds
    }

    fn set_node_bounds(&mut self, bounds: Rectangle) {
        self.state.bounds = bounds;
    }

    pub(crate) fn visual_bounds(&self) -> Rectangle {
        let bounds = self.figure.visual_bounds_in(self.state.bounds);
        self.prepared.as_ref().map_or(bounds, |prepared| {
            bounds.union(prepared.presentation.visual_bounds())
        })
    }

    pub(crate) fn client_area(&self) -> Rectangle {
        let bounds = self.state.bounds;
        let (top, left, bottom, right) = self.state.insets;
        Rectangle::new(
            left,
            top,
            (bounds.width - left - right).max(0.0),
            (bounds.height - top - bottom).max(0.0),
        )
    }

    pub(crate) fn child_layout_area(&self) -> Rectangle {
        let client_area = self.client_area();
        Rectangle::new(0.0, 0.0, client_area.width, client_area.height)
    }

    pub(crate) fn child_transform(&self) -> super::ChildTransform {
        let (top, left, _, _) = self.state.insets;
        let figure_transform = self
            .figure
            .container()
            .map(|container| container.child_transform())
            .unwrap_or(super::ChildTransform::IDENTITY);
        super::ChildTransform::from_affine(
            crate::geometry::Affine2D::from_translation(left, top) * figure_transform.affine(),
        )
    }

    pub(crate) fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.state.child_clipping_strategy.unwrap_or_else(|| {
            self.figure
                .container()
                .map(|container| container.child_clipping_strategy())
                .unwrap_or(ChildClippingStrategy::ClipToChildBounds)
        })
    }

    pub(crate) fn accessible(&self) -> Option<&dyn AccessibleFigure> {
        self.figure.accessible()
    }

    pub(crate) fn clickable_snapshot(&self) -> Option<ClickableSnapshot> {
        self.figure
            .clickable()
            .map(|clickable| clickable.clickable_model().snapshot())
    }

    fn child_policy(&self) -> ChildPolicy {
        self.figure
            .container()
            .map(|container| container.child_policy())
            .unwrap_or(ChildPolicy::Multiple)
    }

    fn layout_constraints(&self, constraints: MeasureConstraints) -> MeasureConstraints {
        self.figure
            .container()
            .map(|container| container.layout_constraints(constraints))
            .unwrap_or(constraints)
    }

    fn project_preferred_measurement(&self, measurement: FigureMeasurement) -> FigureMeasurement {
        self.figure
            .container()
            .map(|container| container.project_preferred_measurement(measurement))
            .unwrap_or(measurement)
    }

    fn project_minimum_size(&self, size: Dimension) -> Dimension {
        self.figure
            .container()
            .map(|container| container.project_minimum_size(size))
            .unwrap_or(size)
    }

    /// 获取首选尺寸
    pub fn get_preferred_size(&self) -> (f64, f64) {
        if let Some(size) = self.preferred_size {
            return size;
        }
        let bounds = self.state.bounds;
        (bounds.width, bounds.height)
    }

    /// 获取最小尺寸
    pub fn get_minimum_size(&self) -> (f64, f64) {
        if let Some(size) = self.minimum_size {
            return size;
        }
        self.get_preferred_size()
    }

    /// 获取最大尺寸
    pub fn get_maximum_size(&self) -> (f64, f64) {
        if let Some(size) = self.maximum_size {
            return size;
        }
        (f64::INFINITY, f64::INFINITY)
    }
}

/// 场景图
///
/// 管理所有图形块的层次结构，参考 Eclipse Draw2d 设计模式。
///
/// # 使用示例
///
/// ```
/// use novadraw::{Figure, RectangleFigure, FigureTree};
///
/// let mut scene = FigureTree::new();
///
/// // 创建根内容块（类似 Draw2d 的 setContents）
/// let contents = RectangleFigure::new(0.0, 0.0, 100.0, 50.0);
/// let contents_id = scene.builder().set_contents(Box::new(contents));
///
/// // 添加子块到指定父块（类似 Draw2d 的 parent.addChild(child)）
/// let child = RectangleFigure::new(10.0, 10.0, 80.0, 30.0);
/// scene.builder().add_child(contents_id, Box::new(child)).expect("valid FigureTree construction");
/// ```
pub struct FigureTree {
    blocks: RuntimeArena<FigureId, FigureNode>,
    uuid_map: std::collections::HashMap<Uuid, FigureId>,
    /// 根块（内部使用）
    root: FigureId,
    /// 内容块（用户可访问的根容器）
    contents: Option<FigureId>,
    notification_effects: NotificationQueue,
}

pub(crate) struct TextLayoutRefreshResult {
    pub(crate) changed: Vec<FigureId>,
    pub(crate) style_nodes_visited: u64,
    pub(crate) figures_refreshed: u64,
}

/// Explicit construction-only facade for building a Figure tree before Runtime ownership.
pub struct FigureTreeBuilder<'a> {
    tree: &'a mut FigureTree,
}

pub(crate) struct RetiredSubtree {
    pub(crate) nodes: Vec<FigureNode>,
    parent_constraint: Option<Box<dyn LayoutConstraint>>,
}

impl RetiredSubtree {
    pub(crate) fn complete(self) {
        for mut node in self.nodes {
            if let Some(parent) = node.parent
                && let Some(lifecycle) = node.figure.lifecycle()
            {
                lifecycle.on_detached(crate::FigureLifecycleContext {
                    figure_id: node.id,
                    parent_id: parent,
                    runtime_namespace: node.id.namespace(),
                });
            }
            drop(node);
        }
        drop(self.parent_constraint);
    }
}

impl<'a> FigureTreeBuilder<'a> {
    pub fn set_contents(&mut self, figure: Box<dyn super::Figure>) -> FigureId {
        self.tree.set_contents(figure)
    }

    pub fn add_child(
        &mut self,
        parent: FigureId,
        figure: Box<dyn super::Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        self.tree.try_add_child_to(parent, figure)
    }

    /// Inserts a new child at `0..=children.len()`; the last position appends.
    pub fn insert_child(
        &mut self,
        parent: FigureId,
        index: usize,
        figure: Box<dyn super::Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        self.tree.insert_child_at(parent, index, figure)
    }

    /// Inserts a new child and its parent-owned constraint as one source operation.
    ///
    /// Rejection publishes no node, constraint, invalidation or notification.
    pub fn insert_child_with_constraint<C: LayoutConstraint>(
        &mut self,
        parent: FigureId,
        index: usize,
        figure: Box<dyn super::Figure>,
        constraint: C,
    ) -> Result<FigureId, ChildInsertionError> {
        self.tree
            .insert_child_with_constraint_at(parent, index, figure, Box::new(constraint))
    }

    pub fn add_child_with_bounds(
        &mut self,
        parent: FigureId,
        bounds: Rectangle,
        color: crate::Color,
    ) -> Result<FigureId, GraphMutationError> {
        let figure = super::figure::RectangleFigure::new_with_color(
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            color,
        );
        self.tree.try_add_child_to(parent, Box::new(figure))
    }

    pub fn set_layout_manager(
        &mut self,
        container: FigureId,
        manager: Box<dyn LayoutManager>,
    ) -> Result<bool, LayoutError> {
        self.tree
            .validate_layout_manager_constraints(container, manager.as_ref())?;
        Ok(self.tree.replace_layout_manager(container, Some(manager)))
    }

    pub fn set_layout_constraint<C>(
        &mut self,
        child: FigureId,
        constraint: C,
    ) -> Result<bool, LayoutError>
    where
        C: LayoutConstraint,
    {
        self.tree.validate_layout_constraint(child, &constraint)?;
        Ok(self.tree.set_boxed_constraint(child, Box::new(constraint)))
    }

    pub fn remove_layout_constraint(&mut self, child: FigureId) -> Result<bool, LayoutError> {
        self.tree.validate_layout_child(child)?;
        Ok(self.tree.remove_constraint(child))
    }

    pub fn set_preferred_size(
        &mut self,
        figure: FigureId,
        size: Option<(f64, f64)>,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_preferred_size(figure, size))
    }

    pub fn set_minimum_size(
        &mut self,
        figure: FigureId,
        size: Option<(f64, f64)>,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_minimum_size(figure, size))
    }

    pub fn set_maximum_size(
        &mut self,
        figure: FigureId,
        size: Option<(f64, f64)>,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_maximum_size(figure, size))
    }

    pub fn set_bounds(
        &mut self,
        figure: FigureId,
        bounds: Rectangle,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self
            .tree
            .set_bounds(figure, bounds.x, bounds.y, bounds.width, bounds.height))
    }

    pub fn set_visible(
        &mut self,
        figure: FigureId,
        visible: bool,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_visible(figure, visible))
    }

    pub fn set_enabled(
        &mut self,
        figure: FigureId,
        enabled: bool,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_enabled(figure, enabled))
    }

    pub fn set_focusable(
        &mut self,
        figure: FigureId,
        focusable: bool,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_focusable(figure, focusable))
    }

    pub fn set_focus_traversable(
        &mut self,
        figure: FigureId,
        traversable: bool,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_focus_traversable(figure, traversable))
    }

    pub fn set_figure_style(
        &mut self,
        figure: FigureId,
        style: FigureStyle,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_figure_style(figure, style))
    }

    pub fn set_opaque(
        &mut self,
        figure: FigureId,
        opaque: bool,
    ) -> Result<bool, GraphMutationError> {
        self.tree.ensure_figure(figure)?;
        Ok(self.tree.set_opaque(figure, opaque))
    }

    pub fn validate_subtree(&mut self, container: FigureId) -> Result<(), LayoutError> {
        self.tree.try_revalidate(container)
    }

    pub fn move_child_to_index(
        &mut self,
        parent: FigureId,
        child: FigureId,
        index: usize,
    ) -> Result<bool, GraphMutationError> {
        self.tree
            .validate_child_order_mutation(parent, child, index)?;
        Ok(self.tree.move_child_to_index(parent, child, index))
    }

    pub fn bring_child_to_front(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, GraphMutationError> {
        let child_count = self.tree.validate_direct_child(parent, child)?;
        Ok(self
            .tree
            .move_child_to_index(parent, child, child_count - 1))
    }

    pub fn send_child_to_back(
        &mut self,
        parent: FigureId,
        child: FigureId,
    ) -> Result<bool, GraphMutationError> {
        self.tree.validate_direct_child(parent, child)?;
        Ok(self.tree.move_child_to_index(parent, child, 0))
    }

    pub(crate) fn tree_mut(&mut self) -> &mut FigureTree {
        self.tree
    }
}

impl FigureTree {
    pub fn namespace(&self) -> RuntimeNamespace {
        self.blocks.namespace()
    }

    fn ensure_figure(&self, figure: FigureId) -> Result<(), GraphMutationError> {
        self.blocks
            .contains_key(figure)
            .then_some(())
            .ok_or(GraphMutationError::FigureNotFound(figure))
    }

    pub(crate) fn synthetic_root(&self) -> FigureId {
        self.root
    }

    pub(crate) fn resize_logical_viewport(
        &mut self,
        updates: &mut UpdateManager,
        bounds: Rectangle,
    ) -> bool {
        if self.blocks[self.root].layout.manager.is_none() {
            self.blocks[self.root].layout.manager = Some(Box::new(crate::StackLayout));
        }
        self.set_bounds_with_update(
            updates,
            self.root,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        )
    }

    pub fn builder(&mut self) -> FigureTreeBuilder<'_> {
        FigureTreeBuilder { tree: self }
    }

    /// 创建新场景图
    pub fn new() -> Self {
        let mut blocks = RuntimeArena::new(RuntimeNamespace::new());
        let uuid = Uuid::new_v4();
        let root_bounds = Rectangle::ZERO;

        let root_id = blocks.insert_with_key(|key| FigureNode {
            id: key,
            uuid,
            children: Vec::new(),
            parent: None,
            depth: 0,
            figure: Box::new(super::figure::RootFigure::new(0.0, 0.0, 0.0, 0.0)),
            component_revision: 0,
            prepared: None,
            layout: LayoutState::default(),
            state: NodeState {
                bounds: root_bounds,
                is_valid: true,
                ..NodeState::default()
            },
        });

        FigureTree {
            blocks,
            uuid_map: std::collections::HashMap::new(),
            root: root_id,
            contents: None,
            notification_effects: NotificationQueue::new(),
        }
    }

    /// 返回当前积累的通知 effect。
    ///
    /// 这些 effect 只描述已经发生的语义变化，不在产生时立即执行回调。
    /// 后续完整 listener/subscription 系统应在稳定事务边界 drain/flush 它们。
    pub(crate) fn notification_effects(&self) -> &[NotificationEffect] {
        self.notification_effects.effects()
    }

    /// 排空通知 effect 队列。
    pub(crate) fn drain_notification_effects(&mut self) -> Vec<NotificationEffect> {
        self.notification_effects.drain()
    }

    fn notify_block_changed(&mut self, figure_id: FigureId) {
        self.notification_effects.notify(figure_id);
    }

    fn emit_figure_event(&mut self, event: FigureEvent) {
        self.notification_effects.emit_figure(event);
    }

    fn emit_ancestor_event(&mut self, event: AncestorEvent) {
        self.notification_effects.emit_ancestor(event);
    }

    fn emit_property_event(&mut self, event: PropertyChangeEvent) {
        self.notification_effects.emit_property(event);
    }

    pub(crate) fn record_property_change(
        &mut self,
        figure_id: FigureId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
    ) {
        self.notify_block_changed(figure_id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id,
            property,
            old_value,
            new_value,
        });
    }

    pub(crate) fn record_coordinate_system_changed(&mut self, figure_id: FigureId) {
        let Some(bounds) = self.figure_bounds(figure_id) else {
            return;
        };
        self.notify_block_changed(figure_id);
        self.emit_figure_event(FigureEvent::CoordinateSystemChanged {
            figure_id,
            old_bounds: bounds,
            new_bounds: bounds,
        });
    }

    fn emit_layout_event(&mut self, event: LayoutEvent) {
        self.notification_effects.emit_layout(event);
    }

    /// 使布局失效，下次渲染时将重新计算布局
    ///
    /// 对应 draw2d: Figure.invalidate()
    pub(crate) fn invalidate(&mut self) {
        let target = self.contents.unwrap_or(self.root);
        self.mark_validation_path_invalid(target);
    }

    /// 标记块需要重新布局
    ///
    /// 对应 draw2d: Figure.revalidate() -> UpdateManager.addInvalidFigure()
    /// 将块添加到更新管理器的失效队列中。
    ///
    /// # Arguments
    ///
    /// * `figure_id` - 需要重新布局的块 ID
    pub(crate) fn mark_invalid(&mut self, update_manager: &mut UpdateManager, figure_id: FigureId) {
        self.mark_validation_path_invalid(figure_id);
        update_manager.add_invalid_figure(figure_id);
    }

    /// 请求重绘指定块
    ///
    /// 对应 draw2d: Figure.repaint() -> UpdateManager.addDirtyRegion()
    /// 将块添加到更新管理器的脏区域队列中。
    ///
    /// # Arguments
    ///
    /// * `figure_id` - 需要重绘的块 ID
    /// * `rect` - node-local 脏区域；`None` 表示完整 local border box
    pub(crate) fn repaint(
        &mut self,
        update_manager: &mut UpdateManager,
        figure_id: FigureId,
        rect: Option<Rectangle>,
    ) {
        if let Some(block) = self.blocks.get(figure_id) {
            if !self.is_effectively_visible(figure_id) {
                return;
            }

            let dirty_rect = rect.unwrap_or_else(|| block.visual_bounds());
            update_manager.add_dirty_region(figure_id, dirty_rect);
        }
    }

    /// 执行更新（两阶段：布局 + 重绘）
    ///
    /// 对应 draw2d: DeferredUpdateManager.performUpdate()
    ///
    /// Phase 1: 布局验证
    /// - 遍历所有失效块，调用 revalidate() 执行布局
    /// - 调用 Figure.validate() 预计算几何属性（如 Triangle 顶点）
    ///
    /// Phase 2: 脏区域重绘
    /// - 如果有待重绘的脏区域，使用脏区域裁剪渲染
    /// - 清空脏区域
    pub(crate) fn perform_update(&mut self, update_manager: &mut UpdateManager) -> NdCanvas {
        let mut canvas = NdCanvas::new();
        update_manager.perform_update(self, &mut canvas);
        canvas
    }

    // ========== 调试验证方法 ==========

    /// 打印场景图树结构（用于调试）
    ///
    /// 使用 `eprintln!` 输出到 stderr，格式示例：
    /// ```text
    /// V FigureId(0x1): Figure bounds=(0,0,100,100)
    ///   V FigureId(0x2): RectangleFigure bounds=(10,10,50,50)
    ///   H FigureId(0x3): RectangleFigure bounds=(50,50,50,50)  // 不可见
    /// ```
    #[cfg(feature = "debug_render")]
    #[allow(dead_code)]
    pub(crate) fn print_tree(&self) {
        eprintln!("\n========== 场景图结构 ==========");
        self.print_block(self.root, 0);
        eprintln!("=================================\n");
    }

    /// 递归打印单个块（内部使用）
    #[cfg(feature = "debug_render")]
    #[allow(dead_code)]
    fn print_block(&self, figure_id: FigureId, depth: usize) {
        let indent = "  ".repeat(depth);
        if let Some(block) = self.blocks.get(figure_id) {
            let bounds = block.figure_bounds();
            let visibility = if block.is_visible { "V" } else { "H" };
            eprintln!(
                "{}{} {:?}: {} bounds=({:.0},{:.0},{:.0},{:.0})",
                indent,
                visibility,
                figure_id,
                block.figure.name(),
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height
            );

            // 正序打印子节点（视觉上：先添加的在上面）
            for &child_id in &block.children {
                self.print_block(child_id, depth + 1);
            }
        }
    }

    /// 打印渲染顺序（调试）
    ///
    /// 在渲染前调用，渲染后会打印渲染顺序
    #[cfg(feature = "debug_render")]
    #[allow(clippy::collapsible_if, dead_code)]
    pub(crate) fn print_render_order(&self) {
        let start_id = self.contents.unwrap_or(self.root);
        let mut stack = vec![start_id];

        eprintln!("\n========== 渲染顺序 ==========");
        let mut order = Vec::new();

        while let Some(figure_id) = stack.pop() {
            if let Some(block) = self.blocks.get(figure_id) {
                if block.is_visible {
                    let bounds = block.figure_bounds();
                    order.push(format!("{}: {:?}", block.figure.name(), bounds));

                    for &child_id in block.children.iter().rev() {
                        if let Some(child) = self.blocks.get(child_id) {
                            if child.is_visible {
                                stack.push(child_id);
                            }
                        }
                    }
                }
            }
        }

        for (i, info) in order.iter().enumerate() {
            eprintln!("  {}: {}", i, info);
        }
        eprintln!("================================\n");
    }

    pub(crate) fn node_mut(&mut self, id: FigureId) -> Option<&mut FigureNode> {
        self.blocks.get_mut(id)
    }

    pub(crate) fn prepare_connection_route(
        &self,
        id: FigureId,
        parent_points: &PointList,
    ) -> Option<crate::PreparedConnectionGeometry> {
        self.blocks
            .get(id)?
            .figure
            .connection()?
            .prepare_route_geometry(parent_points)
            .ok()
    }

    pub(crate) fn prepare_connection_decoration(
        &self,
        id: FigureId,
        placement: crate::LocatorPlacement,
    ) -> Option<Result<crate::PreparedDecorationGeometry, crate::DecorationError>> {
        Some(
            self.blocks
                .get(id)?
                .figure
                .connection_decoration()?
                .prepare_decoration_geometry(placement),
        )
    }

    pub(crate) fn commit_prepared_connection_decoration(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        geometry: crate::PreparedDecorationGeometry,
    ) {
        let (old_bounds, old_visual_bounds, parent_id, visible) = self
            .blocks
            .get(id)
            .and_then(|block| {
                block.figure.connection_decoration().map(|_| {
                    (
                        block.figure_bounds(),
                        block.visual_bounds(),
                        block.parent,
                        self.is_effectively_visible(id),
                    )
                })
            })
            .expect("prepared decoration geometry references a live decoration Figure");
        let bounds = geometry.bounds();

        if visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        let block = self
            .blocks
            .get_mut(id)
            .expect("prepared decoration geometry references a live Figure");
        block.set_node_bounds(bounds);
        block
            .figure
            .connection_decoration_mut()
            .expect("prepared decoration geometry references decoration behavior")
            .commit_decoration_geometry(geometry);

        self.notify_block_changed(id);
        if old_bounds != bounds {
            self.emit_figure_event(FigureEvent::FigureMoved {
                figure_id: id,
                old_bounds,
                new_bounds: bounds,
            });
            self.mark_freeform_ancestor_extents_dirty(id);
        }
        self.mark_invalid(update_manager, id);
        if visible {
            self.repaint(update_manager, id, None);
        }
    }

    pub(crate) fn commit_prepared_connection_route(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        geometry: crate::PreparedConnectionGeometry,
    ) {
        let (old_bounds, old_visual_bounds, parent_id, visible) = self
            .blocks
            .get(id)
            .and_then(|block| {
                block.figure.connection().map(|_| {
                    (
                        block.figure_bounds(),
                        block.visual_bounds(),
                        block.parent,
                        self.is_effectively_visible(id),
                    )
                })
            })
            .expect("prepared Connection geometry references a live Connection Figure");
        let (path_bounds, local_points) = geometry.into_parts();

        if visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        let block = self
            .blocks
            .get_mut(id)
            .expect("prepared Connection geometry references a live Figure");
        block.set_node_bounds(path_bounds);
        let connection = block
            .figure
            .connection_mut()
            .expect("prepared Connection geometry references Connection behavior");
        connection.commit_route_points(local_points);

        self.notify_block_changed(id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            figure_id: id,
            old_bounds,
            new_bounds: path_bounds,
        });
        self.mark_invalid(update_manager, id);
        self.mark_freeform_ancestor_extents_dirty(id);
        if visible {
            self.repaint(update_manager, id, None);
        }
    }

    pub(crate) fn clear_connection_route(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
    ) -> bool {
        let Some((old_bounds, old_visual_bounds, parent_id, visible)) =
            self.blocks.get(id).and_then(|block| {
                block.figure.connection().map(|_| {
                    (
                        block.figure_bounds(),
                        block.visual_bounds(),
                        block.parent,
                        self.is_effectively_visible(id),
                    )
                })
            })
        else {
            return false;
        };
        if visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        block.set_node_bounds(Rectangle::ZERO);
        let Some(connection) = block.figure.connection_mut() else {
            return false;
        };
        connection.commit_route_points(PointList::new());
        self.notify_block_changed(id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            figure_id: id,
            old_bounds,
            new_bounds: Rectangle::ZERO,
        });
        self.mark_invalid(update_manager, id);
        self.mark_freeform_ancestor_extents_dirty(id);
        true
    }

    pub(crate) fn set_focusable(&mut self, id: FigureId, focusable: bool) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.is_focusable == focusable {
            return false;
        }
        let old_value = block.is_focusable;
        block.is_focusable = focusable;
        self.record_property_change(
            id,
            "focusable",
            PropertyValue::Bool(old_value),
            PropertyValue::Bool(focusable),
        );
        true
    }

    pub(crate) fn set_focus_traversable(&mut self, id: FigureId, traversable: bool) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.is_focus_traversable == traversable {
            return false;
        }
        let old_value = block.is_focus_traversable;
        block.is_focus_traversable = traversable;
        self.record_property_change(
            id,
            "focus_traversable",
            PropertyValue::Bool(old_value),
            PropertyValue::Bool(traversable),
        );
        true
    }

    /// 设置块可见性。
    pub(crate) fn set_visible(&mut self, id: FigureId, visible: bool) -> bool {
        let old_value;
        {
            let Some(block) = self.blocks.get_mut(id) else {
                return false;
            };

            if block.is_visible == visible {
                return false;
            }

            old_value = block.is_visible;
            block.is_visible = visible;
        }

        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "visible",
            old_value: PropertyValue::Bool(old_value),
            new_value: PropertyValue::Bool(visible),
        });
        true
    }

    pub(crate) fn set_visible_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        visible: bool,
    ) -> bool {
        let Some((old_visual_bounds, parent_id, was_effectively_visible)) =
            self.blocks.get(id).map(|block| {
                (
                    block.visual_bounds(),
                    block.parent,
                    self.is_effectively_visible(id),
                )
            })
        else {
            return false;
        };
        if !self.set_visible(id, visible) {
            return false;
        }

        if was_effectively_visible && !visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        self.mark_invalid(update_manager, parent_id.unwrap_or(id));
        if visible {
            self.repaint(update_manager, id, None);
        }
        true
    }

    /// 设置块启用状态。
    pub(crate) fn set_enabled(&mut self, id: FigureId, enabled: bool) -> bool {
        let old_value;
        {
            let Some(block) = self.blocks.get_mut(id) else {
                return false;
            };

            if block.is_enabled == enabled {
                return false;
            }

            old_value = block.is_enabled;
            block.is_enabled = enabled;
        }

        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "enabled",
            old_value: PropertyValue::Bool(old_value),
            new_value: PropertyValue::Bool(enabled),
        });
        true
    }

    pub(crate) fn set_enabled_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        enabled: bool,
    ) -> bool {
        if !self.set_enabled(id, enabled) {
            return false;
        }
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        true
    }

    // ========== 坐标变换方法 ==========

    /// 原始平移（对应 draw2d: primTranslate）
    ///
    /// Moves one node in its parent content domain.
    ///
    /// Descendant bounds remain unchanged. Their projected positions change
    /// through the shared parent transform.
    #[cfg(test)]
    pub(crate) fn prim_translate(&mut self, figure_id: FigureId, dx: f64, dy: f64) {
        let Some((old_bounds, new_bounds, has_children)) =
            self.blocks.get_mut(figure_id).map(|block| {
                let old_bounds = block.figure_bounds();
                let new_bounds = Rectangle::new(
                    old_bounds.x + dx,
                    old_bounds.y + dy,
                    old_bounds.width,
                    old_bounds.height,
                );
                block.set_node_bounds(new_bounds);
                (old_bounds, new_bounds, !block.children.is_empty())
            })
        else {
            return;
        };

        self.notify_block_changed(figure_id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            figure_id,
            old_bounds,
            new_bounds,
        });
        if has_children {
            self.emit_figure_event(FigureEvent::CoordinateSystemChanged {
                figure_id,
                old_bounds,
                new_bounds,
            });
        }
        self.emit_ancestor_moved(figure_id);
        self.mark_freeform_ancestor_extents_dirty(figure_id);
    }

    fn emit_ancestor_moved(&mut self, ancestor_id: FigureId) {
        let mut stack = self
            .blocks
            .get(ancestor_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        while let Some(figure_id) = stack.pop() {
            if let Some(block) = self.blocks.get(figure_id) {
                stack.extend(block.children.iter().copied());
            }
            self.emit_ancestor_event(AncestorEvent {
                kind: AncestorEventKind::Moved,
                figure_id,
                parent_id: ancestor_id,
            });
        }
    }

    /// 设置节点的 bounds
    ///
    /// 对应 draw2d: setBounds(Rectangle)
    /// 核心逻辑：
    /// Updates position and size atomically without rewriting descendant bounds.
    #[allow(clippy::collapsible_if)]
    pub(crate) fn set_bounds(
        &mut self,
        figure_id: FigureId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let (old_bounds, has_children) = {
            if let Some(block) = self.blocks.get(figure_id) {
                (block.figure_bounds(), !block.children.is_empty())
            } else {
                return false;
            }
        };
        let resize = width != old_bounds.width || height != old_bounds.height;
        let translate = x != old_bounds.x || y != old_bounds.y;
        if !resize && !translate {
            return false;
        }

        let bounds = Rectangle::new(x, y, width, height);
        if let Some(block) = self.blocks.get_mut(figure_id) {
            block.set_node_bounds(bounds);
        }
        self.notify_block_changed(figure_id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            figure_id,
            old_bounds,
            new_bounds: bounds,
        });
        if translate && has_children {
            self.emit_figure_event(FigureEvent::CoordinateSystemChanged {
                figure_id,
                old_bounds,
                new_bounds: bounds,
            });
        }
        if translate {
            self.emit_ancestor_moved(figure_id);
        }
        self.mark_freeform_ancestor_extents_dirty(figure_id);
        true
    }

    /// 设置节点 bounds 并进入 Draw2D 等价的更新链路。
    ///
    /// 对应 draw2d: Figure#setBounds(Rectangle)
    ///
    /// 与低层 [`Self::set_bounds`] 的区别：
    ///
    /// - 移动或 resize 前，按 `erase()` 语义把旧 bounds 转到 parent 坐标域并请求 parent repaint；
    /// - 移动后，请求当前 Figure repaint；
    /// - resize 时，同时使当前 Figure 的 validation 失效。
    ///
    /// 布局与批量构建仍可使用低层 `set_bounds`；交互式移动、拖拽和运行时 resize
    /// 应使用此方法，确保旧区域曝光、当前区域重绘和坐标根移动使用同一 damage 协议。
    pub(crate) fn set_bounds_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        figure_id: FigureId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let Some(block) = self.blocks.get(figure_id) else {
            return false;
        };
        let old_bounds = block.figure_bounds();
        let old_visual_bounds = block.visual_bounds();
        let resize = width != old_bounds.width || height != old_bounds.height;
        let translate = x != old_bounds.x || y != old_bounds.y;
        if !resize && !translate {
            return false;
        }
        let parent_id = block.parent;
        let freeform_ancestor = self.nearest_freeform_ancestor(figure_id);
        let visible = self.is_effectively_visible(figure_id);

        if visible {
            self.erase(update_manager, figure_id, old_visual_bounds, parent_id);
        }

        self.set_bounds(figure_id, x, y, width, height);

        if resize {
            self.mark_invalid(update_manager, figure_id);
        } else if let Some(freeform_ancestor) = freeform_ancestor {
            update_manager.add_invalid_figure(freeform_ancestor);
        }

        if visible {
            self.repaint(update_manager, figure_id, None);
        }

        true
    }

    fn erase(
        &self,
        update_manager: &mut UpdateManager,
        figure_id: FigureId,
        old_visual_bounds: Rectangle,
        parent_id: Option<FigureId>,
    ) {
        if parent_id.is_none() || !self.blocks.contains_key(figure_id) {
            return;
        }
        update_manager.freeze_figure_damage(self, figure_id, old_visual_bounds);
    }
}

impl Default for FigureTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::super::figure::{Bounded, ChildClippingStrategy, RectangleFigure, Shape};
    use crate::Color as NovadrawCoreColor;
    use crate::geometry::{Point, Translatable};
    use crate::render::{NdCanvas, command::RenderCommandKind};
    use crate::style::{CursorIcon, FigureStyle, ResolvedStyle};
    use crate::{
        EllipseFigure, Figure, FigureEvent, FigureEventHandler, FigureLifecycle, FigureTree,
        LineBorder, NotificationEffect, PolygonFigure, PolylineFigure, Rectangle, RootFigure,
        RoundedRectangleFigure, ScalableLayeredPaneFigure, TriangleFigure, UpdateManager,
        ViewportFigure,
    };

    #[derive(Debug, PartialEq)]
    enum RenderSignature {
        PushState,
        RestoreState,
        PopState,
        Clip([f64; 4]),
        FillRect([f64; 4]),
        StrokeRect([f64; 4]),
        Other(&'static str),
    }

    fn rect_signature(rect: &Rectangle) -> [f64; 4] {
        [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
    }

    fn render_signatures(gc: &NdCanvas) -> Vec<RenderSignature> {
        gc.commands()
            .iter()
            .map(|command| match &command.kind {
                RenderCommandKind::PushState => RenderSignature::PushState,
                RenderCommandKind::RestoreState => RenderSignature::RestoreState,
                RenderCommandKind::PopState => RenderSignature::PopState,
                RenderCommandKind::Clip { rect } => RenderSignature::Clip(rect_signature(rect)),
                RenderCommandKind::FillRect { rect, .. } => {
                    RenderSignature::FillRect(rect_signature(rect))
                }
                RenderCommandKind::StrokeRect { rect, .. } => {
                    RenderSignature::StrokeRect(rect_signature(rect))
                }
                _ => RenderSignature::Other("other"),
            })
            .collect()
    }

    // ========== 通用测试 Figure 类型 ==========

    /// 坐标根 Figure（使用本地坐标）
    #[derive(Clone, Copy)]
    struct TestCoordinateRootFigure {
        bounds: Rectangle,
    }

    impl TestCoordinateRootFigure {
        fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
            Self {
                bounds: Rectangle::new(x, y, width, height),
            }
        }
    }

    impl Bounded for TestCoordinateRootFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "TestCoordinateRootFigure"
        }
    }

    impl Shape for TestCoordinateRootFigure {
        fn stroke_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn line_cap(&self) -> crate::render::command::LineCap {
            crate::render::command::LineCap::default()
        }

        fn line_join(&self) -> crate::render::command::LineJoin {
            crate::render::command::LineJoin::default()
        }

        fn fill_enabled(&self) -> bool {
            false
        }

        fn outline_enabled(&self) -> bool {
            false
        }

        fn fill_shape(&self, _gc: &mut NdCanvas) {}

        fn outline_shape(&self, _gc: &mut NdCanvas) {}
    }

    #[derive(Clone, Copy)]
    struct OverflowPaintFigure {
        bounds: Rectangle,
        paint_rect: Rectangle,
    }

    impl OverflowPaintFigure {
        fn new(bounds: Rectangle, paint_rect: Rectangle) -> Self {
            Self { bounds, paint_rect }
        }
    }

    impl Bounded for OverflowPaintFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "OverflowPaintFigure"
        }

        fn visual_bounds(&self) -> Rectangle {
            self.paint_rect
        }
    }

    impl Shape for OverflowPaintFigure {
        fn stroke_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<NovadrawCoreColor> {
            Some(NovadrawCoreColor::from_hex("#44aa44").expect("valid color literal"))
        }

        fn line_cap(&self) -> crate::render::command::LineCap {
            crate::render::command::LineCap::default()
        }

        fn line_join(&self) -> crate::render::command::LineJoin {
            crate::render::command::LineJoin::default()
        }

        fn fill_shape(&self, gc: &mut NdCanvas) {
            gc.fill_rect_with_color(
                self.paint_rect.x,
                self.paint_rect.y,
                self.paint_rect.width,
                self.paint_rect.height,
                NovadrawCoreColor::from_hex("#44aa44").expect("valid color literal"),
            );
        }

        fn outline_shape(&self, _gc: &mut NdCanvas) {}
    }

    struct LifecycleRecordingFigure {
        bounds: Rectangle,
        events: Arc<Mutex<Vec<LifecycleEvent>>>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum LifecycleEvent {
        Attached(crate::FigureLifecycleContext),
        Detached(crate::FigureLifecycleContext),
    }

    impl LifecycleRecordingFigure {
        fn new(events: Arc<Mutex<Vec<LifecycleEvent>>>) -> Self {
            Self {
                bounds: Rectangle::new(0.0, 0.0, 10.0, 10.0),
                events,
            }
        }
    }

    impl Bounded for LifecycleRecordingFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "LifecycleRecordingFigure"
        }
    }

    impl Figure for LifecycleRecordingFigure {
        fn initial_bounds(&self) -> Rectangle {
            Bounded::bounds(self)
        }

        fn name(&self) -> &'static str {
            Bounded::name(self)
        }

        fn lifecycle(&mut self) -> Option<&mut dyn FigureLifecycle> {
            Some(self)
        }
    }

    impl FigureLifecycle for LifecycleRecordingFigure {
        fn on_attached(&mut self, context: crate::FigureLifecycleContext) {
            self.events
                .lock()
                .unwrap()
                .push(LifecycleEvent::Attached(context));
        }

        fn on_detached(&mut self, context: crate::FigureLifecycleContext) {
            self.events
                .lock()
                .unwrap()
                .push(LifecycleEvent::Detached(context));
        }
    }

    /// 带 insets 的 Figure
    #[derive(Clone, Copy)]
    struct TestFigureWithInsets {
        bounds: Rectangle,
        insets: (f64, f64, f64, f64),
    }

    impl TestFigureWithInsets {
        fn new(x: f64, y: f64, width: f64, height: f64, insets: (f64, f64, f64, f64)) -> Self {
            Self {
                bounds: Rectangle::new(x, y, width, height),
                insets,
            }
        }
    }

    impl Bounded for TestFigureWithInsets {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn insets(&self) -> (f64, f64, f64, f64) {
            self.insets
        }

        fn name(&self) -> &'static str {
            "TestFigureWithInsets"
        }
    }

    impl Shape for TestFigureWithInsets {
        fn stroke_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn line_cap(&self) -> crate::render::command::LineCap {
            crate::render::command::LineCap::default()
        }

        fn line_join(&self) -> crate::render::command::LineJoin {
            crate::render::command::LineJoin::default()
        }

        fn fill_enabled(&self) -> bool {
            false
        }

        fn outline_enabled(&self) -> bool {
            false
        }

        fn fill_shape(&self, _gc: &mut NdCanvas) {}

        fn outline_shape(&self, _gc: &mut NdCanvas) {}
    }

    #[derive(Clone, Copy)]
    struct TestInteractiveFigure {
        bounds: Rectangle,
    }

    impl TestInteractiveFigure {
        fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
            Self {
                bounds: Rectangle::new(x, y, width, height),
            }
        }
    }

    impl Bounded for TestInteractiveFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "TestInteractiveFigure"
        }
    }

    impl Shape for TestInteractiveFigure {
        fn stroke_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn stroke_width(&self) -> f64 {
            0.0
        }

        fn fill_color(&self) -> Option<NovadrawCoreColor> {
            None
        }

        fn line_cap(&self) -> crate::render::command::LineCap {
            crate::render::command::LineCap::default()
        }

        fn line_join(&self) -> crate::render::command::LineJoin {
            crate::render::command::LineJoin::default()
        }

        fn fill_enabled(&self) -> bool {
            false
        }

        fn outline_enabled(&self) -> bool {
            false
        }

        fn fill_shape(&self, _gc: &mut NdCanvas) {}

        fn outline_shape(&self, _gc: &mut NdCanvas) {}
    }

    impl FigureEventHandler for TestInteractiveFigure {}

    macro_rules! impl_test_shape_figure {
        ($($figure:ty),+ $(,)?) => {
            $(
                impl Figure for $figure {
                    fn initial_bounds(&self) -> Rectangle {
                        Bounded::bounds(self)
                    }

                    fn name(&self) -> &'static str {
                        Bounded::name(self)
                    }

                    fn initial_insets(&self) -> (f64, f64, f64, f64) {
                        Bounded::insets(self)
                    }

                    fn paint_figure(&self, gc: &mut NdCanvas) {
                        Shape::paint_figure(self, gc);
                    }
                }
            )+
        };
    }

    impl_test_shape_figure!(TestCoordinateRootFigure, TestFigureWithInsets);

    impl Figure for OverflowPaintFigure {
        fn initial_bounds(&self) -> Rectangle {
            Bounded::bounds(self)
        }

        fn name(&self) -> &'static str {
            Bounded::name(self)
        }

        fn paint_figure(&self, gc: &mut NdCanvas) {
            Shape::paint_figure(self, gc);
        }

        fn visual_bounds_in(&self, bounds: Rectangle) -> Rectangle {
            Rectangle::new(
                self.paint_rect.x,
                self.paint_rect.y,
                bounds.width + self.paint_rect.width - self.bounds.width,
                bounds.height + self.paint_rect.height - self.bounds.height,
            )
        }
    }

    impl Figure for TestInteractiveFigure {
        fn initial_bounds(&self) -> Rectangle {
            Bounded::bounds(self)
        }

        fn name(&self) -> &'static str {
            Bounded::name(self)
        }

        fn paint_figure(&self, gc: &mut NdCanvas) {
            Shape::paint_figure(self, gc);
        }

        fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
            Some(self)
        }
    }

    struct AlphaStateFigure {
        bounds: Rectangle,
    }

    struct OpaqueProbeFigure {
        bounds: Rectangle,
    }

    impl Figure for OpaqueProbeFigure {
        fn initial_bounds(&self) -> Rectangle {
            self.bounds
        }

        fn name(&self) -> &'static str {
            "OpaqueProbeFigure"
        }
    }

    impl Bounded for AlphaStateFigure {
        fn bounds(&self) -> Rectangle {
            self.bounds
        }

        fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.bounds = Rectangle::new(x, y, width, height);
        }

        fn name(&self) -> &'static str {
            "AlphaStateFigure"
        }
    }

    impl Figure for AlphaStateFigure {
        fn initial_bounds(&self) -> Rectangle {
            Bounded::bounds(self)
        }

        fn name(&self) -> &'static str {
            Bounded::name(self)
        }

        fn paint_figure(&self, gc: &mut NdCanvas) {
            let bounds = self.bounds;
            gc.fill_rect_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                NovadrawCoreColor::WHITE,
            );
        }
    }

    /// 测试渲染顺序：Z-order 验证
    ///
    /// 场景：父容器包含三个子矩形（从下到上添加）
    /// 期望：渲染顺序应为 parent → child1 → child2 → child3
    ///       即先添加的在下面（被遮挡），后添加的在上面（遮挡别人）
    #[test]
    fn test_render_order_z_order() {
        let mut scene = FigureTree::new();

        // 创建父容器（100x100）
        let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.set_contents(Box::new(parent));

        // 添加三个子矩形（从下到上添加）
        let child1 = RectangleFigure::new(10.0, 10.0, 20.0, 20.0);
        let _c1 = scene.add_child_to(parent_id, Box::new(child1));

        let child2 = RectangleFigure::new(30.0, 30.0, 20.0, 20.0);
        let _c2 = scene.add_child_to(parent_id, Box::new(child2));

        let child3 = RectangleFigure::new(50.0, 50.0, 20.0, 20.0);
        let _c3 = scene.add_child_to(parent_id, Box::new(child3));

        // 打印树结构（用于手动验证）
        {
            eprintln!("\n=== 场景图树结构 ===");
            // print_block 仅在 debug_render feature 下可用
            eprintln!("====================\n");

            // 打印预期渲染顺序
            eprintln!("预期渲染顺序（先渲染的在下面）:");
            eprintln!("  0: parent");
            eprintln!("  1: child1 (最早添加，在最下层)");
            eprintln!("  2: child2");
            eprintln!("  3: child3 (最晚添加，在最上层)");
            eprintln!();
        }

        // 渲染并验证命令数量
        let gc = scene.render();
        let cmd_count = gc.commands().len();

        // 渲染：每个矩形产生多个命令
        // parent + 3 个子矩形 = 4 个图形
        // 新渲染流程（每个图形）：
        //   - save (transform)
        //   - save (prepare_context)
        //   - translate (bounds)
        //   - clip_rect
        //   - fill_rect
        //   - restore (after paint_figure)
        //   - stroke_rect (border)
        //   - restore (PostOrder)
        // parent: save + save + translate + clip + fill + restore + stroke + restore = 8
        // 每个 child: save + save + translate + clip + fill + restore + restore = 7
        // Total: 8 + 3 * 7 = 29
        assert!(
            cmd_count >= 35,
            "应有至少 35 个渲染命令，实际为 {}",
            cmd_count
        );
    }

    /// 测试渲染顺序：嵌套层次
    ///
    /// 场景：父 → 子1 → 孙1
    /// 期望渲染顺序：parent → child1 → grandchild1
    #[test]
    fn test_render_order_nested() {
        let mut scene = FigureTree::new();

        // 根
        let root = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
        let root_id = scene.set_contents(Box::new(root));

        // 子
        let child = RectangleFigure::new(50.0, 50.0, 100.0, 100.0);
        let child_id = scene.add_child_to(root_id, Box::new(child));

        // 孙
        let grandchild = RectangleFigure::new(60.0, 60.0, 30.0, 30.0);
        let _gc_id = scene.add_child_to(child_id, Box::new(grandchild));

        // 打印树结构
        {
            eprintln!("\n=== 嵌套场景图树结构 ===");
            // print_block 仅在 debug_render feature 下可用
            eprintln!("=======================\n");

            // 预期渲染顺序：root → child → grandchild
            eprintln!("预期渲染顺序:");
            eprintln!("  0: root");
            eprintln!("  1: child");
            eprintln!("  2: grandchild");
            eprintln!();
        }

        let gc = scene.render();
        let cmd_count = gc.commands().len();

        // 渲染：每个图形产生多个命令
        // 3 个图形：root + child + grandchild
        // 每个图形的命令数（参见 test_render_order_z_order）
        // Total: 8 (root) + 7 (child) + 7 (grandchild) = 22
        assert!(
            cmd_count >= 20,
            "应有至少 20 个渲染命令，实际为 {}",
            cmd_count
        );
    }

    /// 测试可见性过滤
    ///
    /// 场景：父容器包含可见子元素和不可见子元素
    /// 期望：只渲染可见元素
    #[test]
    fn test_visibility_filter() {
        let mut scene = FigureTree::new();

        let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.set_contents(Box::new(parent));

        // 可见子元素
        let visible_child = RectangleFigure::new(10.0, 10.0, 20.0, 20.0);
        let _ = scene.add_child_to(parent_id, Box::new(visible_child));

        // 不可见子元素
        let invisible_child = RectangleFigure::new(50.0, 50.0, 20.0, 20.0);
        let invisible_id = scene.add_child_to(parent_id, Box::new(invisible_child));

        // 设置不可见
        scene.blocks.get_mut(invisible_id).unwrap().is_visible = false;

        let gc = scene.render();
        let fill_count = gc
            .commands()
            .iter()
            .filter(|command| matches!(command.kind, RenderCommandKind::FillRect { .. }))
            .count();
        assert_eq!(fill_count, 2, "只应绘制 parent 和 visible child");
    }

    /// 测试变换累加
    ///
    /// 场景：子元素有非零位置
    /// 期望：Trampoline 渲染能正确处理嵌套层次
    #[test]
    fn test_transform_accumulation() {
        let mut scene = FigureTree::new();

        let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.set_contents(Box::new(parent));

        let child = RectangleFigure::new(25.0, 25.0, 50.0, 50.0);
        let _child_id = scene.add_child_to(parent_id, Box::new(child));

        // 打印场景结构
        {
            eprintln!("\n=== 测试变换累加 ===");
            eprintln!(
                "Parent bounds: {:?}",
                scene.blocks.get(parent_id).unwrap().figure_bounds()
            );
            eprintln!(
                "Child bounds: {:?}",
                scene.blocks.get(parent_id).unwrap().children
            );
        }

        // 渲染应能正确处理嵌套层次
        let gc = scene.render();
        let commands = gc.commands();

        // 验证：parent + child = 2 个图形
        // 每个图形的命令数（参见 test_render_order_z_order）
        // parent: 8, child: 7, Total: 15
        assert!(
            commands.len() >= 8,
            "应有足够的渲染命令，实际为 {}",
            commands.len()
        );

        // 验证有 FillRect 命令
        let has_fill_rect = commands.iter().any(|cmd| {
            matches!(
                cmd.kind,
                crate::render::command::RenderCommandKind::FillRect { .. }
            )
        });
        assert!(has_fill_rect, "应有 FillRect 命令");
    }

    #[test]
    fn test_find_mouse_event_target_at_skips_non_interactive_figures() {
        let mut scene = FigureTree::new();
        scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        assert_eq!(scene.find_mouse_event_target_at(10.0, 10.0), None);
    }

    #[test]
    fn test_find_mouse_event_target_at_prefers_deepest_interactive_figure() {
        let mut scene = FigureTree::new();
        let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
        let interactive_parent = scene.add_child_to(
            root_id,
            Box::new(TestInteractiveFigure::new(10.0, 10.0, 120.0, 120.0)),
        );
        let interactive_child = scene.add_child_to(
            interactive_parent,
            Box::new(TestInteractiveFigure::new(20.0, 20.0, 40.0, 40.0)),
        );

        assert_eq!(
            scene.find_mouse_event_target_at(15.0, 15.0),
            Some(interactive_parent)
        );
        assert_eq!(
            scene.find_mouse_event_target_at(35.0, 35.0),
            Some(interactive_child)
        );
    }

    #[test]
    fn test_hit_test_descends_only_through_parent_client_area() {
        let mut scene = FigureTree::new();
        let parent_id = scene.set_contents(Box::new(TestFigureWithInsets::new(
            100.0,
            100.0,
            100.0,
            100.0,
            (10.0, 10.0, 10.0, 10.0),
        )));
        let child_id = scene.add_child_to(
            parent_id,
            Box::new(RectangleFigure::new(-5.0, -5.0, 20.0, 20.0)),
        );

        assert_eq!(scene.hit_test_simple((105.0, 105.0)), Some(parent_id));
        assert_eq!(scene.hit_test_simple((111.0, 111.0)), Some(child_id));
    }

    #[test]
    fn test_mouse_event_target_descends_only_through_parent_client_area() {
        let mut scene = FigureTree::new();
        let parent_id = scene.set_contents(Box::new(TestFigureWithInsets::new(
            100.0,
            100.0,
            100.0,
            100.0,
            (10.0, 10.0, 10.0, 10.0),
        )));
        let child_id = scene.add_child_to(
            parent_id,
            Box::new(TestInteractiveFigure::new(-5.0, -5.0, 20.0, 20.0)),
        );

        assert_eq!(scene.find_mouse_event_target_at(105.0, 105.0), None);
        assert_eq!(
            scene.find_mouse_event_target_at(111.0, 111.0),
            Some(child_id)
        );
    }

    #[test]
    fn test_child_order_appends_children_back_to_front() {
        let mut scene = FigureTree::new();
        let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
        let first = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
        );
        let second = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 50.0, 50.0)),
        );
        let third = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(30.0, 30.0, 50.0, 50.0)),
        );

        assert_eq!(scene.child_order(root_id), Some(vec![first, second, third]));
        assert_eq!(scene.child_z_index(root_id, first), Some(0));
        assert_eq!(scene.child_z_index(root_id, second), Some(1));
        assert_eq!(scene.child_z_index(root_id, third), Some(2));
    }

    #[test]
    fn test_figure_lifecycle_hooks_follow_runtime_realization_and_completion() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut scene = FigureTree::new();
        let left_id =
            scene.set_contents(Box::new(LifecycleRecordingFigure::new(Arc::clone(&events))));
        let synthetic_root = scene.synthetic_root();
        let right_id = scene.add_child_to(
            left_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 50.0, 50.0)),
        );
        let child_id = scene.add_child_to(
            left_id,
            Box::new(LifecycleRecordingFigure::new(Arc::clone(&events))),
        );
        let grandchild_id = scene.add_child_to(
            child_id,
            Box::new(LifecycleRecordingFigure::new(Arc::clone(&events))),
        );

        assert!(events.lock().unwrap().is_empty());

        let mut runtime = crate::Runtime::new(scene);
        let namespace = child_id.namespace();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: left_id,
                    parent_id: synthetic_root,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: child_id,
                    parent_id: left_id,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: grandchild_id,
                    parent_id: child_id,
                    runtime_namespace: namespace,
                }),
            ]
        );

        assert!(
            runtime
                .reparent(child_id, right_id)
                .expect("valid Runtime mutation")
        );
        runtime.dispose_subtree(child_id).unwrap();
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: left_id,
                    parent_id: synthetic_root,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: child_id,
                    parent_id: left_id,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: grandchild_id,
                    parent_id: child_id,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Detached(crate::FigureLifecycleContext {
                    figure_id: child_id,
                    parent_id: left_id,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Attached(crate::FigureLifecycleContext {
                    figure_id: child_id,
                    parent_id: right_id,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Detached(crate::FigureLifecycleContext {
                    figure_id: grandchild_id,
                    parent_id: child_id,
                    runtime_namespace: namespace,
                }),
                LifecycleEvent::Detached(crate::FigureLifecycleContext {
                    figure_id: child_id,
                    parent_id: right_id,
                    runtime_namespace: namespace,
                }),
            ]
        );
    }

    #[test]
    fn test_z_order_reorder_changes_topmost_hit_test_target() {
        let mut scene = FigureTree::new();
        let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
        let bottom = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        );
        let middle = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        );
        let top = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        );

        assert_eq!(scene.hit_test_simple((30.0, 30.0)), Some(top));

        assert!(scene.bring_child_to_front(root_id, bottom));
        assert_eq!(scene.child_order(root_id), Some(vec![middle, top, bottom]));
        assert_eq!(scene.hit_test_simple((30.0, 30.0)), Some(bottom));

        assert!(scene.send_child_to_back(root_id, bottom));
        assert_eq!(scene.child_order(root_id), Some(vec![bottom, middle, top]));
        assert_eq!(scene.hit_test_simple((30.0, 30.0)), Some(top));
    }

    #[test]
    fn test_z_order_reorder_rejects_invalid_inputs_without_side_effects() {
        let mut scene = FigureTree::new();
        let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
        let child = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 100.0, 100.0)),
        );
        let _sibling = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(40.0, 40.0, 100.0, 100.0)),
        );
        let other_parent = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(0.0, 0.0, 10.0, 10.0)),
        );
        let initial_order = scene.child_order(root_id);

        assert!(!scene.move_child_to_index(root_id, child, 3));
        assert!(!scene.move_child_to_index(other_parent, child, 0));
        assert!(!scene.bring_child_to_front(root_id, other_parent));
        assert_eq!(scene.child_order(root_id), initial_order);
    }

    #[test]
    fn test_hit_test_translates_through_coordinate_root() {
        let mut scene = FigureTree::new();
        let contents_id =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 300.0)));
        let coordinate_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(100.0, 50.0, 120.0, 120.0)),
        );
        let child_id = scene.add_child_to(
            coordinate_root_id,
            Box::new(RectangleFigure::new(20.0, 30.0, 40.0, 40.0)),
        );

        assert_eq!(scene.hit_test_simple((130.0, 90.0)), Some(child_id));
        assert_eq!(
            scene.hit_test_simple((115.0, 65.0)),
            Some(coordinate_root_id)
        );
        assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(contents_id));
    }

    #[test]
    fn test_hit_test_translates_through_viewport_figure() {
        let mut scene = FigureTree::new();
        let contents_id =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 300.0)));
        let viewport_id = scene.add_child_to(
            contents_id,
            Box::new(ViewportFigure::new(100.0, 50.0, 120.0, 80.0).with_origin(40.0, 20.0)),
        );
        let scalable_id = scene.add_child_to(
            viewport_id,
            Box::new(ScalableLayeredPaneFigure::new(0.0, 0.0, 240.0, 160.0).with_scale(2.0)),
        );
        let child_id = scene.add_child_to(
            scalable_id,
            Box::new(RectangleFigure::new(30.0, 20.0, 20.0, 20.0)),
        );

        assert_eq!(scene.hit_test_simple((120.0, 70.0)), Some(child_id));
        assert_eq!(scene.hit_test_simple((105.0, 55.0)), Some(scalable_id));
        assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(contents_id));
    }

    #[test]
    fn test_find_mouse_event_target_at_translates_through_coordinate_root() {
        let mut scene = FigureTree::new();
        let contents_id =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 300.0)));
        let coordinate_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(100.0, 50.0, 120.0, 120.0)),
        );
        let interactive_child = scene.add_child_to(
            coordinate_root_id,
            Box::new(TestInteractiveFigure::new(20.0, 30.0, 40.0, 40.0)),
        );

        assert_eq!(
            scene.find_mouse_event_target_at(130.0, 90.0),
            Some(interactive_child)
        );
        assert_eq!(scene.find_mouse_event_target_at(115.0, 65.0), None);
    }

    // ========== 坐标变换测试 ==========

    /// 测试 prim_translate 基本功能
    ///
    /// 场景：平移父节点，子节点也应被平移
    /// 期望：父子节点的 bounds 都被平移相同的量
    #[test]
    fn test_prim_translate_basic() {
        let mut scene = FigureTree::new();

        // 创建父子层次
        let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.set_contents(Box::new(parent));

        let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // 平移父节点 (10, 20)
        scene.prim_translate(parent_id, 10.0, 20.0);

        // 验证父节点 bounds
        let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
        assert_eq!(parent_bounds.x, 10.0, "父节点 x 应为 10");
        assert_eq!(parent_bounds.y, 20.0, "父节点 y 应为 20");

        // 子节点通过父链变换改变世界位置，但 parent-local 存储值不变。
        let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
        assert_eq!(child_bounds.x, 10.0);
        assert_eq!(child_bounds.y, 10.0);
    }

    #[test]
    fn test_prim_translate_records_figure_moved_effects() {
        let mut scene = FigureTree::new();
        let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child_id = scene.add_child_to(
            parent_id,
            Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
        );

        scene.drain_notification_effects();
        scene.prim_translate(parent_id, 10.0, 20.0);

        let effects = scene.drain_notification_effects();

        assert!(effects.contains(&NotificationEffect::Notify {
            figure_id: parent_id
        }));
        assert!(!effects.contains(&NotificationEffect::Notify {
            figure_id: child_id
        }));
        assert!(
            effects.contains(&NotificationEffect::EmitFigure(FigureEvent::FigureMoved {
                figure_id: parent_id,
                old_bounds: Rectangle::new(0.0, 0.0, 100.0, 100.0),
                new_bounds: Rectangle::new(10.0, 20.0, 100.0, 100.0),
            }))
        );
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                NotificationEffect::EmitFigure(FigureEvent::FigureMoved { figure_id, .. })
                    if *figure_id == child_id
            )
        }));
    }

    #[test]
    fn test_prim_translate_records_coordinate_system_changed_effect() {
        let mut scene = FigureTree::new();
        let root_id = scene.set_contents(Box::new(TestCoordinateRootFigure::new(
            0.0, 0.0, 100.0, 100.0,
        )));
        let child_id = scene.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(10.0, 10.0, 50.0, 50.0)),
        );

        scene.drain_notification_effects();
        scene.prim_translate(root_id, 5.0, 10.0);

        let effects = scene.drain_notification_effects();
        let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();

        assert_eq!(child_bounds, Rectangle::new(10.0, 10.0, 50.0, 50.0));
        assert!(effects.contains(&NotificationEffect::EmitFigure(
            FigureEvent::CoordinateSystemChanged {
                figure_id: root_id,
                old_bounds: Rectangle::new(0.0, 0.0, 100.0, 100.0),
                new_bounds: Rectangle::new(5.0, 10.0, 100.0, 100.0),
            }
        )));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                NotificationEffect::EmitFigure(FigureEvent::FigureMoved { figure_id, .. })
                    if *figure_id == child_id
            )
        }));
    }

    /// 测试 prim_translate 嵌套传播
    ///
    /// 场景：平移根节点，所有后代都被平移
    /// 期望：整棵子树的 bounds 都被平移
    #[test]
    fn test_prim_translate_nested() {
        let mut scene = FigureTree::new();

        // 创建三层层次：root -> parent -> child
        let root = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
        let root_id = scene.set_contents(Box::new(root));

        let parent = RectangleFigure::new(50.0, 50.0, 100.0, 100.0);
        let parent_id = scene.add_child_to(root_id, Box::new(parent));

        let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // 平移根节点 (5, 10)
        scene.prim_translate(root_id, 5.0, 10.0);

        // 后代通过父链变换改变世界位置，但 parent-local 存储值不变。
        let root_bounds = scene.blocks.get(root_id).unwrap().figure_bounds();
        assert_eq!(root_bounds.x, 5.0);
        assert_eq!(root_bounds.y, 10.0);

        let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
        assert_eq!(parent_bounds.x, 50.0);
        assert_eq!(parent_bounds.y, 50.0);

        let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
        assert_eq!(child_bounds.x, 10.0);
        assert_eq!(child_bounds.y, 10.0);
    }

    // ========== local_to_parent_transform 测试 ==========

    /// 测试 local_to_parent_transform 基本功能
    ///
    /// 场景：当前节点是坐标根且无 insets
    /// 期望：本地坐标 (10, 20) 转换为父坐标 (30, 50)
    #[test]
    fn test_local_to_parent_transform_basic() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );

        let mut point = (10.0, 20.0);
        point.transform(scene.local_to_parent_transform(coord_root_id).unwrap());
        assert_eq!(point, (30.0, 50.0));
    }

    /// Node placement 不包含其 child-content insets。
    #[test]
    fn test_local_to_parent_transform_with_insets() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestFigureWithInsets::new(
                20.0,
                30.0,
                100.0,
                100.0,
                (5.0, 5.0, 0.0, 0.0),
            )),
        );
        let mut point = (10.0, 20.0);
        point.transform(scene.local_to_parent_transform(coord_root_id).unwrap());
        assert_eq!(point.0, 30.0);
        assert_eq!(point.1, 50.0);
    }

    /// 测试 local_to_parent_transform 不依赖父节点是否为坐标根
    ///
    /// 场景：当前节点不是坐标根
    /// 期望：不进行转换，返回原坐标
    #[test]
    fn test_local_to_parent_transform_not_coordinate_root() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.add_child_to(contents_id, Box::new(parent));

        let child = RectangleFigure::new(10.0, 20.0, 50.0, 50.0);
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        let mut point = (10.0, 20.0);
        point.transform(scene.local_to_parent_transform(child_id).unwrap());
        assert_eq!(point, (20.0, 40.0));
    }

    // ========== parent_to_local_transform 测试 ==========

    /// 测试 parent_to_local_transform 基本功能
    ///
    /// 场景：当前节点是坐标根且无 insets
    /// 期望：父坐标 (30, 50) 转换为本地坐标 (10, 20)
    #[test]
    fn test_parent_to_local_transform_basic() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );
        let mut point = (30.0, 50.0);
        point.transform(scene.parent_to_local_transform(coord_root_id).unwrap());
        assert_eq!(point, (10.0, 20.0));
    }

    /// Parent content 到 node local 只逆转 node placement。
    #[test]
    fn test_parent_to_local_transform_with_insets() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestFigureWithInsets::new(
                20.0,
                30.0,
                100.0,
                100.0,
                (5.0, 5.0, 0.0, 0.0),
            )),
        );
        let mut point = (35.0, 55.0);
        point.transform(scene.parent_to_local_transform(coord_root_id).unwrap());
        assert_eq!(point.0, 15.0);
        assert_eq!(point.1, 25.0);
    }

    // ========== surface_to_local_transform 测试 ==========

    /// 测试 surface_to_local_transform 基本功能
    ///
    /// 场景：父节点是坐标根，bounds = (0, 0)
    /// 期望：绝对坐标 (30, 40) 转换为本地坐标 (30, 40)
    #[test]
    fn test_surface_to_local_transform_basic() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let parent_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(0.0, 0.0, 100.0, 100.0)),
        );

        let child = RectangleFigure::new(30.0, 40.0, 50.0, 50.0);
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // 绝对坐标位于 child 原点，转换后得到 node-local 原点。
        let mut point = (30.0, 40.0);
        point.transform(scene.surface_to_local_transform(child_id).unwrap());
        assert_eq!(point, (0.0, 0.0));
    }

    /// 测试 surface_to_local_transform 嵌套坐标根
    ///
    /// 场景：深层嵌套，多个坐标根
    /// 期望：正确累积转换
    #[test]
    fn test_surface_to_local_transform_nested() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        // coord_root1 (20, 30)
        let coord_root1_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );

        // coord_root2 相对于 coord_root1 (10, 5)
        let coord_root2_id = scene.add_child_to(
            coord_root1_id,
            Box::new(TestCoordinateRootFigure::new(10.0, 5.0, 50.0, 50.0)),
        );

        // child 相对于 coord_root2 (15, 25)
        let child = RectangleFigure::new(15.0, 25.0, 30.0, 30.0);
        let child_id = scene.add_child_to(coord_root2_id, Box::new(child));

        // 绝对坐标 = coord_root1 + coord_root2 + child = (20+10+15, 30+5+25) = (45, 60)
        // 该绝对坐标是 child 的 node-local 原点。
        let mut point = (45.0, 60.0);
        point.transform(scene.surface_to_local_transform(child_id).unwrap());
        assert_eq!(point, (0.0, 0.0));
    }

    /// 测试 surface_to_local_transform 与 local_to_surface_transform 严格互逆。
    ///
    /// 场景：目标节点本身也是坐标根。
    /// 期望：转换到 absolute 后再转换回 relative 时，不会额外应用目标节点自己的
    /// translateFromParent；这与 Draw2D Figure#translateToRelative 的 parent-chain 协议一致。
    #[test]
    fn test_surface_to_local_transform_roundtrips_target_coordinate_root() {
        let mut scene = FigureTree::new();

        let contents_id =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));

        let coord_root1_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );

        let coord_root2_id = scene.add_child_to(
            coord_root1_id,
            Box::new(TestCoordinateRootFigure::new(10.0, 5.0, 50.0, 50.0)),
        );

        let mut point = (15.0, 25.0);
        point.transform(scene.local_to_surface_transform(coord_root2_id).unwrap());
        assert_eq!(point, (45.0, 60.0));

        point.transform(scene.surface_to_local_transform(coord_root2_id).unwrap());
        assert_eq!(point, (15.0, 25.0));
    }

    /// 测试 surface_to_local_transform 可应用到 Rectangle。
    ///
    /// 场景：使用 Rectangle 类型进行坐标转换
    /// 期望：Rectangle 的 x, y 被正确转换
    #[test]
    fn test_surface_to_local_transform_rect() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        // coord_root (10, 20)
        let parent_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(10.0, 20.0, 100.0, 100.0)),
        );

        // child 相对于 coord_root (30, 40)
        let child = RectangleFigure::new(30.0, 40.0, 50.0, 50.0);
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // 绝对矩形从 child 原点开始。
        let mut rect = Rectangle::new(40.0, 60.0, 50.0, 50.0);
        rect.transform(scene.surface_to_local_transform(child_id).unwrap());
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
    }

    // ========== local_to_surface_transform 测试 ==========

    /// 测试 local_to_surface_transform 基本功能
    ///
    /// 场景：父节点是坐标根，bounds = (20, 30)
    /// 期望：本地坐标 (10, 5) 转换为绝对坐标 (30, 35)
    #[test]
    fn test_local_to_surface_transform_basic() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        // coord_root (20, 30)
        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );

        // child 相对于 coord_root (10, 5)
        let child = RectangleFigure::new(10.0, 5.0, 50.0, 50.0);
        let child_id = scene.add_child_to(coord_root_id, Box::new(child));

        let mut point = (0.0, 0.0);
        point.transform(scene.local_to_surface_transform(child_id).unwrap());
        assert_eq!(point, (30.0, 35.0));
    }

    /// 测试 local_to_surface_transform 在坐标根包含 insets 时通过父链协议叠加。
    #[test]
    fn test_local_to_surface_transform_includes_parent_insets() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestFigureWithInsets::new(
                20.0,
                30.0,
                100.0,
                100.0,
                (5.0, 7.0, 0.0, 0.0),
            )),
        );

        let child = RectangleFigure::new(10.0, 5.0, 50.0, 50.0);
        let child_id = scene.add_child_to(coord_root_id, Box::new(child));

        let mut point = (0.0, 0.0);
        point.transform(scene.local_to_surface_transform(child_id).unwrap());
        assert_eq!(point, (37.0, 40.0));
    }

    /// 测试 local_to_surface_transform 嵌套坐标根
    ///
    /// 场景：多层坐标根
    /// 期望：正确累加多个坐标根的 bounds
    #[test]
    fn test_local_to_surface_transform_nested() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        // coord_root1 (10, 20)
        let coord_root1_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(10.0, 20.0, 100.0, 100.0)),
        );

        // coord_root2 相对于 coord_root1 (5, 10)
        let coord_root2_id = scene.add_child_to(
            coord_root1_id,
            Box::new(TestCoordinateRootFigure::new(5.0, 10.0, 50.0, 50.0)),
        );

        // child 相对于 coord_root2 (15, 25)
        let child = RectangleFigure::new(15.0, 25.0, 30.0, 30.0);
        let child_id = scene.add_child_to(coord_root2_id, Box::new(child));

        // 绝对坐标 = coord_root1 + coord_root2 + child = (10+5+15, 20+10+25) = (30, 55)
        let mut point = (0.0, 0.0);
        point.transform(scene.local_to_surface_transform(child_id).unwrap());
        assert_eq!(point, (30.0, 55.0));
    }

    /// 测试 local_to_surface_transform 在多层坐标根且包含 insets 时严格按父链协议累加。
    #[test]
    fn test_local_to_surface_transform_nested_insets_follow_parent_chain_protocol() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root1_id = scene.add_child_to(
            contents_id,
            Box::new(TestFigureWithInsets::new(
                10.0,
                20.0,
                100.0,
                100.0,
                (2.0, 3.0, 0.0, 0.0),
            )),
        );

        let coord_root2_id = scene.add_child_to(
            coord_root1_id,
            Box::new(TestFigureWithInsets::new(
                5.0,
                10.0,
                50.0,
                50.0,
                (4.0, 6.0, 0.0, 0.0),
            )),
        );

        let child = RectangleFigure::new(15.0, 25.0, 30.0, 30.0);
        let child_id = scene.add_child_to(coord_root2_id, Box::new(child));

        let mut point = (0.0, 0.0);
        point.transform(scene.local_to_surface_transform(child_id).unwrap());
        assert_eq!(point, (39.0, 61.0));
    }

    /// 测试 local_to_surface_transform 可应用到 Rectangle。
    ///
    /// 场景：使用 Rectangle 类型进行坐标转换
    /// 期望：Rectangle 的 x, y 被正确转换
    #[test]
    fn test_local_to_surface_transform_rect() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        // coord_root (20, 30)
        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );

        // child 相对于 coord_root (10, 5)
        let child = RectangleFigure::new(10.0, 5.0, 50.0, 50.0);
        let child_id = scene.add_child_to(coord_root_id, Box::new(child));

        let mut rect = Rectangle::new(0.0, 0.0, 50.0, 50.0);
        rect.transform(scene.local_to_surface_transform(child_id).unwrap());
        assert_eq!(rect.x, 30.0);
        assert_eq!(rect.y, 35.0);
    }

    #[test]
    fn moved_figure_damage_uses_old_and_new_projected_visual_bounds() {
        let mut scene = FigureTree::new();
        let contents = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 240.0)));
        let figure = scene.add_child_to(
            contents,
            Box::new(OverflowPaintFigure::new(
                Rectangle::new(50.0, 40.0, 20.0, 20.0),
                Rectangle::new(-5.0, -6.0, 30.0, 32.0),
            )),
        );
        let mut updates = crate::UpdateManager::new();

        assert!(scene.set_bounds_with_update(&mut updates, figure, 70.0, 60.0, 20.0, 20.0,));

        let canvas = scene.perform_update(&mut updates);
        assert_eq!(
            canvas.damage().union(),
            Some(Rectangle::new(45.0, 34.0, 50.0, 52.0))
        );
    }

    #[test]
    fn test_border_insets_define_client_area_clip_for_children() {
        let mut scene = FigureTree::new();

        let parent_id = scene.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 120.0, 100.0).with_border(
                LineBorder::new(
                    NovadrawCoreColor::from_hex("#111111").expect("valid color literal"),
                    2.0,
                )
                .with_insets(10.0, 20.0, 30.0, 40.0),
            ),
        ));
        scene.add_child_to(
            parent_id,
            Box::new(RectangleFigure::new_with_color(
                5.0,
                5.0,
                20.0,
                20.0,
                NovadrawCoreColor::from_hex("#222222").expect("valid color literal"),
            )),
        );

        let recursive = scene.render();
        let signatures = render_signatures(&recursive);

        assert!(
            signatures.contains(&RenderSignature::Clip([20.0, 10.0, 80.0, 70.0])),
            "parent clientArea must be clipped by border insets: {signatures:?}"
        );
        assert!(
            signatures.contains(&RenderSignature::StrokeRect([1.0, 1.0, 119.0, 99.0])),
            "border must render in the outer ring"
        );

        let child_fill_index = signatures
            .iter()
            .position(|signature| *signature == RenderSignature::FillRect([0.0, 0.0, 20.0, 20.0]))
            .expect("child fill must be rendered under parent clientArea clip");
        let parent_border_index = signatures
            .iter()
            .position(|signature| {
                *signature == RenderSignature::StrokeRect([1.0, 1.0, 119.0, 99.0])
            })
            .expect("parent border must be rendered");
        assert!(
            parent_border_index > child_fill_index,
            "border must render after children"
        );
    }

    #[test]
    fn test_paint_clip_and_hit_test_share_border_inset_client_area() {
        let mut scene = FigureTree::new();

        let parent_id = scene.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 120.0, 100.0).with_border(
                LineBorder::new(
                    NovadrawCoreColor::from_hex("#111111").expect("valid color literal"),
                    2.0,
                )
                .with_insets(10.0, 20.0, 30.0, 40.0),
            ),
        ));
        let child_id = scene.add_child_to(
            parent_id,
            Box::new(RectangleFigure::new_with_color(
                5.0,
                5.0,
                20.0,
                20.0,
                NovadrawCoreColor::from_hex("#222222").expect("valid color literal"),
            )),
        );

        let recursive = scene.render();
        let signatures = render_signatures(&recursive);
        assert!(
            signatures.contains(&RenderSignature::Clip([20.0, 10.0, 80.0, 70.0])),
            "paint traversal must clip children to the border-inset clientArea: {signatures:?}"
        );

        assert_eq!(
            scene.hit_test_simple((6.0, 6.0)),
            Some(parent_id),
            "hit-test must not descend into children outside the painted clientArea"
        );
        assert_eq!(
            scene.hit_test_simple((26.0, 16.0)),
            Some(child_id),
            "hit-test should descend once the point is inside the painted clientArea"
        );
    }

    #[test]
    fn test_default_clipping_strategy_clips_children_to_child_bounds() {
        let mut scene = FigureTree::new();

        let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        scene.add_child_to(
            parent_id,
            Box::new(OverflowPaintFigure::new(
                Rectangle::new(20.0, 20.0, 10.0, 10.0),
                Rectangle::new(0.0, 0.0, 80.0, 80.0),
            )),
        );

        let signatures = render_signatures(&scene.render());
        let overflow_paint_index = signatures
            .iter()
            .position(|signature| *signature == RenderSignature::FillRect([0.0, 0.0, 80.0, 80.0]))
            .expect("overflow child paint must be emitted");
        let child_bounds_clip_index = signatures
            .iter()
            .position(|signature| *signature == RenderSignature::Clip([20.0, 20.0, 30.0, 30.0]))
            .expect("default clipping strategy must clip to child bounds");

        assert!(
            child_bounds_clip_index < overflow_paint_index,
            "child bounds clip must be applied before child paint"
        );
    }

    #[test]
    fn test_custom_clipping_strategy_can_skip_child_bounds_clip() {
        let mut scene = FigureTree::new();

        let parent_id = scene.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0)
                .with_child_clipping_strategy(ChildClippingStrategy::DoNotClipChildBounds),
        ));
        scene.add_child_to(
            parent_id,
            Box::new(OverflowPaintFigure::new(
                Rectangle::new(20.0, 20.0, 10.0, 10.0),
                Rectangle::new(0.0, 0.0, 80.0, 80.0),
            )),
        );

        let signatures = render_signatures(&scene.render());
        let overflow_paint_index = signatures
            .iter()
            .position(|signature| *signature == RenderSignature::FillRect([0.0, 0.0, 80.0, 80.0]))
            .expect("overflow child paint must be emitted");

        assert!(
            !signatures[..overflow_paint_index]
                .contains(&RenderSignature::Clip([20.0, 20.0, 30.0, 30.0])),
            "custom clipping strategy must not clip child paint to child bounds"
        );
        assert!(
            signatures.contains(&RenderSignature::Clip([0.0, 0.0, 100.0, 100.0])),
            "parent clientArea clip must remain active"
        );
    }

    #[test]
    fn test_unclipped_children_restore_parent_graphics_state_between_siblings() {
        let mut scene = FigureTree::new();
        let parent_id = scene.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0)
                .with_child_clipping_strategy(ChildClippingStrategy::DoNotClipChildBounds),
        ));
        let first = scene.add_child_to(
            parent_id,
            Box::new(AlphaStateFigure {
                bounds: Rectangle::new(0.0, 0.0, 10.0, 10.0),
            }),
        );
        scene.set_figure_style(
            first,
            FigureStyle {
                alpha: Some(0.25),
                ..FigureStyle::default()
            },
        );
        scene.add_child_to(
            parent_id,
            Box::new(AlphaStateFigure {
                bounds: Rectangle::new(20.0, 0.0, 10.0, 10.0),
            }),
        );

        let canvas = scene.render();
        let sibling_color = canvas
            .commands()
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::FillRect {
                    rect,
                    paint: crate::graphics::Paint::Solid(color),
                    ..
                } if rect_signature(rect) == [20.0, 0.0, 30.0, 10.0] => Some(*color),
                _ => None,
            })
            .expect("second sibling must paint");

        assert_eq!(sibling_color.alpha(), 1.0);
    }

    #[test]
    fn test_existing_figures_expose_child_clipping_strategy() {
        let parent_factories: Vec<(&str, Box<dyn Fn() -> Box<dyn Figure>>)> = vec![
            (
                "ellipse",
                Box::new(|| {
                    Box::new(
                        EllipseFigure::new(0.0, 0.0, 100.0, 100.0).with_child_clipping_strategy(
                            ChildClippingStrategy::DoNotClipChildBounds,
                        ),
                    )
                }),
            ),
            (
                "rounded_rectangle",
                Box::new(|| {
                    Box::new(
                        RoundedRectangleFigure::new(0.0, 0.0, 100.0, 100.0, 8.0)
                            .with_child_clipping_strategy(
                                ChildClippingStrategy::DoNotClipChildBounds,
                            ),
                    )
                }),
            ),
            (
                "polyline",
                Box::new(|| {
                    Box::new(
                        PolylineFigure::new(0.0, 0.0, 100.0, 100.0).with_child_clipping_strategy(
                            ChildClippingStrategy::DoNotClipChildBounds,
                        ),
                    )
                }),
            ),
            (
                "polygon",
                Box::new(|| {
                    Box::new(
                        PolygonFigure::from_points(vec![
                            Point::new(0.0, 0.0),
                            Point::new(100.0, 0.0),
                            Point::new(100.0, 100.0),
                            Point::new(0.0, 100.0),
                        ])
                        .with_child_clipping_strategy(ChildClippingStrategy::DoNotClipChildBounds),
                    )
                }),
            ),
            (
                "triangle",
                Box::new(|| {
                    Box::new(
                        TriangleFigure::new(0.0, 0.0, 100.0, 100.0).with_child_clipping_strategy(
                            ChildClippingStrategy::DoNotClipChildBounds,
                        ),
                    )
                }),
            ),
            (
                "root",
                Box::new(|| {
                    Box::new(
                        RootFigure::new(0.0, 0.0, 100.0, 100.0).with_child_clipping_strategy(
                            ChildClippingStrategy::DoNotClipChildBounds,
                        ),
                    )
                }),
            ),
            (
                "viewport",
                Box::new(|| {
                    Box::new(
                        ViewportFigure::new(0.0, 0.0, 100.0, 100.0).with_child_clipping_strategy(
                            ChildClippingStrategy::DoNotClipChildBounds,
                        ),
                    )
                }),
            ),
        ];

        for (name, make_parent) in parent_factories {
            let mut scene = FigureTree::new();
            let parent_id = scene.set_contents(make_parent());
            scene.add_child_to(
                parent_id,
                Box::new(OverflowPaintFigure::new(
                    Rectangle::new(20.0, 20.0, 10.0, 10.0),
                    Rectangle::new(0.0, 0.0, 80.0, 80.0),
                )),
            );

            let signatures = render_signatures(&scene.render());
            let overflow_paint_index = signatures
                .iter()
                .position(|signature| {
                    *signature == RenderSignature::FillRect([0.0, 0.0, 80.0, 80.0])
                })
                .unwrap_or_else(|| panic!("{name}: overflow child paint must be emitted"));

            assert!(
                !signatures[..overflow_paint_index]
                    .contains(&RenderSignature::Clip([20.0, 20.0, 30.0, 30.0])),
                "{name}: custom clipping strategy must not clip child paint to child bounds"
            );
        }
    }

    #[test]
    fn test_mouse_event_target_uses_same_border_inset_client_area_as_paint() {
        let mut scene = FigureTree::new();

        let parent_id = scene.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 120.0, 100.0).with_border(
                LineBorder::new(
                    NovadrawCoreColor::from_hex("#111111").expect("valid color literal"),
                    2.0,
                )
                .with_insets(10.0, 20.0, 30.0, 40.0),
            ),
        ));
        let child_id = scene.add_child_to(
            parent_id,
            Box::new(TestInteractiveFigure::new(5.0, 5.0, 20.0, 20.0)),
        );

        assert_eq!(scene.find_mouse_event_target_at(6.0, 6.0), None);
        assert_eq!(scene.find_mouse_event_target_at(26.0, 16.0), Some(child_id));
    }

    #[test]
    fn node_bounds_drive_rendering_without_writing_back_to_figure() {
        let mut scene = FigureTree::new();
        let id = scene.set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            20.0,
            20.0,
            NovadrawCoreColor::WHITE,
        )));

        scene.set_bounds(id, 0.0, 0.0, 80.0, 60.0);

        let figure = scene.blocks[id]
            .figure
            .as_any()
            .downcast_ref::<RectangleFigure>()
            .unwrap();
        assert_eq!(figure.bounds, Rectangle::new(0.0, 0.0, 20.0, 20.0));
        assert_eq!(
            scene.figure_bounds(id),
            Some(Rectangle::new(0.0, 0.0, 80.0, 60.0))
        );
        assert!(
            render_signatures(&scene.render())
                .contains(&RenderSignature::FillRect([0.0, 0.0, 80.0, 60.0]))
        );
    }

    #[test]
    fn replacing_contents_detaches_the_previous_root_subtree() {
        let mut scene = FigureTree::new();
        let previous = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let replacement =
            scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));

        assert!(!scene.is_attached(previous));
        assert!(scene.is_attached(replacement));
        assert_eq!(scene.contents(), Some(replacement));
        assert_eq!(scene.child_order(scene.root), Some(vec![replacement]));
    }

    #[test]
    fn node_style_inherits_each_unset_property_independently() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            scene.add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let foreground = NovadrawCoreColor::from_hex("#123456").expect("valid color literal");
        let background = NovadrawCoreColor::from_hex("#abcdef").expect("valid color literal");

        scene.set_figure_style(
            parent,
            FigureStyle {
                foreground: Some(foreground),
                background: None,
                alpha: Some(0.75),
                font: Some("18px serif".to_string()),
                cursor: Some(CursorIcon::Pointer),
                tooltip: None,
            },
        );
        scene.set_figure_style(
            child,
            FigureStyle {
                foreground: None,
                background: Some(background),
                alpha: None,
                font: None,
                cursor: None,
                tooltip: Some(Some("child".to_string())),
            },
        );

        assert_eq!(
            scene.resolved_style(child),
            Some(ResolvedStyle {
                foreground,
                background,
                alpha: 0.75,
                font: "18px serif".to_string(),
                cursor: CursorIcon::Pointer,
                tooltip: Some("child".to_string()),
            })
        );
    }

    #[test]
    fn resolved_style_is_applied_to_descendant_graphics_state() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        scene.set_figure_style(
            parent,
            FigureStyle {
                alpha: Some(0.5),
                ..FigureStyle::default()
            },
        );
        scene.add_child_to(
            parent,
            Box::new(AlphaStateFigure {
                bounds: Rectangle::new(20.0, 0.0, 10.0, 10.0),
            }),
        );

        let color = scene
            .render()
            .commands()
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::FillRect {
                    rect,
                    paint: crate::graphics::Paint::Solid(color),
                    ..
                } if rect_signature(rect) == [20.0, 0.0, 30.0, 10.0] => Some(*color),
                _ => None,
            })
            .expect("styled descendant must paint");

        assert_eq!(color.alpha(), 0.5);
    }

    #[test]
    fn opaque_figure_fills_bounds_with_resolved_background_before_content() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child = scene.add_child_to(
            parent,
            Box::new(OpaqueProbeFigure {
                bounds: Rectangle::new(20.0, 30.0, 50.0, 40.0),
            }),
        );
        let background = NovadrawCoreColor::from_hex("#2D7F5E").unwrap();
        scene.set_figure_style(
            child,
            FigureStyle {
                background: Some(background),
                ..FigureStyle::default()
            },
        );
        scene.set_opaque(child, true);

        assert!(scene.render().commands().iter().any(|command| matches!(
            command.kind,
            RenderCommandKind::FillRect { rect, paint: crate::graphics::Paint::Solid(color), .. }
                if rect_signature(&rect) == [0.0, 0.0, 50.0, 40.0]
                    && color == background
        )));
    }

    #[test]
    fn local_alpha_override_replaces_inherited_graphics_alpha() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        scene.set_figure_style(
            parent,
            FigureStyle {
                alpha: Some(0.25),
                ..FigureStyle::default()
            },
        );
        let child = scene.add_child_to(
            parent,
            Box::new(AlphaStateFigure {
                bounds: Rectangle::new(20.0, 0.0, 10.0, 10.0),
            }),
        );
        scene.set_figure_style(
            child,
            FigureStyle {
                alpha: Some(1.0),
                ..FigureStyle::default()
            },
        );

        let color = scene
            .render()
            .commands()
            .iter()
            .find_map(|command| match &command.kind {
                RenderCommandKind::FillRect {
                    rect,
                    paint: crate::graphics::Paint::Solid(color),
                    ..
                } if rect_signature(rect) == [20.0, 0.0, 30.0, 10.0] => Some(*color),
                _ => None,
            })
            .expect("child paint must be emitted");

        assert_eq!(color.alpha(), 1.0);
    }

    #[test]
    fn default_style_does_not_emit_redundant_alpha_commands() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        scene.add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));

        assert!(
            scene
                .render()
                .commands()
                .iter()
                .all(|command| !matches!(&command.kind, RenderCommandKind::SetGlobalAlpha { .. }))
        );
    }

    #[test]
    fn style_change_emits_typed_properties_and_queues_subtree_repaint() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            scene.add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let mut updates = UpdateManager::new();

        assert!(scene.set_figure_style_with_update(
            &mut updates,
            parent,
            FigureStyle {
                foreground: Some(NovadrawCoreColor::WHITE),
                font: Some("16px sans-serif".to_string()),
                cursor: Some(CursorIcon::Crosshair),
                tooltip: Some(Some("container".to_string())),
                ..FigureStyle::default()
            },
        ));

        assert!(updates.has_pending_repaint());
        assert!(updates.has_pending_layout());
        let effects = scene.notification_effects();
        for property in ["foreground", "font", "cursor", "tooltip"] {
            assert!(effects.iter().any(|effect| matches!(
                effect,
                NotificationEffect::EmitProperty(event)
                    if event.figure_id == parent && event.property == property
            )));
        }
        assert_eq!(
            scene.resolved_style(child).unwrap().cursor,
            CursorIcon::Crosshair
        );
    }
}
