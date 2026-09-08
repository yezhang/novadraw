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

use novadraw_geometry::{Affine2D, Dimension, PointList, Rectangle, Translatable, Vec2};
use novadraw_render::{NdCanvas, TextError, TextLayoutEngine};
use slotmap::{Key, SlotMap};
use uuid::Uuid;

use super::figure::{
    ChildClippingStrategy, ChildPolicy, ClickableSnapshot, ClickableVisualState, Direction,
    ImageFigure, LabelFigure, RoundedRectangleFigure, ShapeMutationError, TriangleFigure,
    WidgetError, normalize_points,
};
use super::layout::{
    LayoutChange, LayoutConstraint, LayoutError, LayoutInvalidation, LayoutManager, LayoutOutput,
    LayoutSnapshot,
};
use crate::Border;
use crate::figure::border::BorderSnapshot;
use crate::mutation::{PendingMutation, PendingMutationKind};
use crate::runtime::update::{
    ActionEvent, AncestorEvent, AncestorEventKind, FigureEvent, LayoutEvent, LayoutEventKind,
    NotificationEffect, NotificationQueue, PropertyChangeEvent, PropertyValue, UpdateManager,
};
use crate::style::{FigureStyle, ResolvedStyle};

// 渲染模块
mod render_recursive;
mod search;

use render_recursive::{FigureRenderer, FigureTreeRenderRef};
pub use search::{ExclusionSearch, IdentitySearch, TreeQueryError, TreeSearch, TreeSearchContext};

#[cfg(test)]
pub mod bounds_test;

#[cfg(test)]
pub mod update_integration_test;

slotmap::new_key_type! {
    /// Runtime-local, generational identity of a Figure node.
    pub struct FigureId;
}

/// Figure 树允许的最大深度。根节点深度为 0。
pub const MAX_TREE_DEPTH: usize = 10_000;
pub const DEFAULT_VALIDATION_BUDGET: usize = 10_000;
pub const FREEFORM_EXTENT_PROPERTY: &str = "freeform_extent";

fn finite_rectangle(rectangle: Rectangle) -> bool {
    rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
}

fn transform_rectangle(transform: Affine2D, rectangle: Rectangle) -> Option<Rectangle> {
    let corners = [
        transform.transform_point(rectangle.x, rectangle.y),
        transform.transform_point(rectangle.x + rectangle.width, rectangle.y),
        transform.transform_point(rectangle.x, rectangle.y + rectangle.height),
        transform.transform_point(
            rectangle.x + rectangle.width,
            rectangle.y + rectangle.height,
        ),
    ];
    if corners
        .iter()
        .flat_map(|(x, y)| [x, y])
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let left = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::INFINITY, f64::min);
    let top = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::INFINITY, f64::min);
    let right = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);
    Some(Rectangle::new(left, top, right - left, bottom - top))
}

/// Figure 树结构变更失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMutationError {
    ParentNotFound,
    ChildNotFound,
    CycleDetected,
    DuplicateChild,
    ChildLimitExceeded { limit: usize },
    LayerKeyRequired,
    LayerChildRequired,
    InvalidParentRelation,
    DepthLimitExceeded { limit: usize },
}

impl fmt::Display for GraphMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentNotFound => write!(f, "parent block does not exist"),
            Self::ChildNotFound => write!(f, "child block does not exist"),
            Self::CycleDetected => write!(f, "mutation would create a cycle"),
            Self::DuplicateChild => write!(f, "child is already attached to parent"),
            Self::ChildLimitExceeded { limit } => {
                write!(f, "parent accepts at most {limit} direct child")
            }
            Self::LayerKeyRequired => write!(f, "layered pane mutations require a layer key"),
            Self::LayerChildRequired => write!(f, "layered pane accepts only Layer figures"),
            Self::InvalidParentRelation => write!(f, "child is not attached to expected parent"),
            Self::DepthLimitExceeded { limit } => {
                write!(f, "figure tree depth exceeds limit {limit}")
            }
        }
    }
}

impl Error for GraphMutationError {}

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

fn point_in_rect(point: (f64, f64), rect: &Rectangle) -> bool {
    point.0 >= rect.x
        && point.0 <= rect.x + rect.width
        && point.1 >= rect.y
        && point.1 <= rect.y + rect.height
}

fn owner_scoped_border_size(content: (f64, f64), snapshot: Option<&BorderSnapshot>) -> (f64, f64) {
    let Some(snapshot) = snapshot else {
        return content;
    };
    let (top, left, bottom, right) = snapshot.insets();
    let preferred = snapshot.preferred_size();
    (
        (content.0 + left + right).max(preferred.0),
        (content.1 + top + bottom).max(preferred.1),
    )
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
    w_hint: f64,
    h_hint: f64,
    size: (f64, f64),
}

impl CachedMeasurement {
    fn matches(self, generation: u64, w_hint: f64, h_hint: f64) -> bool {
        self.generation == generation && self.w_hint == w_hint && self.h_hint == h_hint
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
/// 场景图中的基本单元，同时包含图形数据（通过 Box<dyn Figure>）
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
        self.figure.visual_bounds_in(self.state.bounds)
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

    pub(crate) fn child_transform(&self) -> super::ChildTransform {
        let (top, left, _, _) = self.state.insets;
        let figure_transform = self
            .figure
            .container()
            .map(|container| container.child_transform())
            .unwrap_or(super::ChildTransform::IDENTITY);
        super::ChildTransform::from_affine(
            novadraw_geometry::Affine2D::from_translation(left, top) * figure_transform.affine(),
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

    fn child_policy(&self) -> ChildPolicy {
        self.figure
            .container()
            .map(|container| container.child_policy())
            .unwrap_or(ChildPolicy::Multiple)
    }

    fn layout_size_hints(&self, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.figure
            .container()
            .map(|container| container.layout_size_hints(w_hint, h_hint))
            .unwrap_or((w_hint, h_hint))
    }

    fn project_preferred_size(&self, size: (f64, f64)) -> (f64, f64) {
        self.figure
            .container()
            .map(|container| container.project_preferred_size(size))
            .unwrap_or(size)
    }

    fn project_minimum_size(&self, size: (f64, f64)) -> (f64, f64) {
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
/// use novadraw_scene::{Figure, RectangleFigure, FigureTree};
///
/// let mut scene = FigureTree::new();
///
/// // 创建根内容块（类似 Draw2d 的 setContents）
/// let contents = RectangleFigure::new(0.0, 0.0, 100.0, 50.0);
/// let contents_id = scene.builder().set_contents(Box::new(contents));
///
/// // 添加子块到指定父块（类似 Draw2d 的 parent.addChild(child)）
/// let child = RectangleFigure::new(10.0, 10.0, 80.0, 30.0);
/// scene.builder().add_child_to(contents_id, Box::new(child));
/// ```
pub struct FigureTree {
    blocks: SlotMap<FigureId, FigureNode>,
    uuid_map: std::collections::HashMap<Uuid, FigureId>,
    /// 根块（内部使用）
    root: FigureId,
    /// 内容块（用户可访问的根容器）
    contents: Option<FigureId>,
    notification_effects: NotificationQueue,
}

/// Explicit construction-only facade for building a Figure tree before Runtime ownership.
pub struct FigureTreeBuilder<'a> {
    tree: &'a mut FigureTree,
}

impl<'a> FigureTreeBuilder<'a> {
    pub fn set_contents(&mut self, figure: Box<dyn super::Figure>) -> FigureId {
        self.tree.set_contents(figure)
    }

    pub fn add_child_to(&mut self, parent: FigureId, figure: Box<dyn super::Figure>) -> FigureId {
        self.tree.add_child_to(parent, figure)
    }

    pub fn try_add_child_to(
        &mut self,
        parent: FigureId,
        figure: Box<dyn super::Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        self.tree.try_add_child_to(parent, figure)
    }

    pub fn add_child_with_bounds(
        &mut self,
        parent: FigureId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: novadraw_core::Color,
    ) -> FigureId {
        self.tree
            .add_child_with_bounds(parent, x, y, width, height, color)
    }

    pub fn move_child_to_index(&mut self, parent: FigureId, child: FigureId, index: usize) -> bool {
        self.tree.move_child_to_index(parent, child, index)
    }

    pub fn bring_child_to_front(&mut self, parent: FigureId, child: FigureId) -> bool {
        self.tree.bring_child_to_front(parent, child)
    }

    pub fn send_child_to_back(&mut self, parent: FigureId, child: FigureId) -> bool {
        self.tree.send_child_to_back(parent, child)
    }

    pub(crate) fn tree_mut(&mut self) -> &mut FigureTree {
        self.tree
    }
}

impl FigureTree {
    pub fn builder(&mut self) -> FigureTreeBuilder<'_> {
        FigureTreeBuilder { tree: self }
    }

    /// 创建新场景图
    pub fn new() -> Self {
        let mut blocks = SlotMap::with_key();
        let uuid = Uuid::new_v4();
        let root_bounds = Rectangle::ZERO;

        let root_id = blocks.insert_with_key(|key| FigureNode {
            id: key,
            uuid,
            children: Vec::new(),
            parent: None,
            depth: 0,
            figure: Box::new(super::figure::RootFigure::new(0.0, 0.0, 0.0, 0.0)),
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
    pub fn notification_effects(&self) -> &[NotificationEffect] {
        self.notification_effects.effects()
    }

    /// 排空通知 effect 队列。
    pub fn drain_notification_effects(&mut self) -> Vec<NotificationEffect> {
        self.notification_effects.drain()
    }

    fn notify_block_changed(&mut self, block_id: FigureId) {
        self.notification_effects.notify(block_id);
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
        block_id: FigureId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
    ) {
        self.notify_block_changed(block_id);
        self.emit_property_event(PropertyChangeEvent {
            block_id,
            property,
            old_value,
            new_value,
        });
    }

    pub(crate) fn record_coordinate_system_changed(&mut self, block_id: FigureId) {
        let Some(bounds) = self.figure_bounds(block_id) else {
            return;
        };
        self.notify_block_changed(block_id);
        self.emit_figure_event(FigureEvent::CoordinateSystemChanged {
            block_id,
            old_bounds: bounds,
            new_bounds: bounds,
        });
    }

    fn emit_layout_event(&mut self, event: LayoutEvent) {
        self.notification_effects.emit_layout(event);
    }

    /// 设置内容块
    ///
    /// 对应 draw2d: LightweightSystem.setContents(IFigure)
    ///
    /// 设置场景的根容器，后续添加的子块将作为此容器的子元素。
    /// 注意：此方法不触发 revalidate()，用于批量构建场景。
    /// 交互式修改使用 SceneManager.set_contents() 方法。
    pub(crate) fn set_contents(&mut self, figure: Box<dyn super::Figure>) -> FigureId {
        if let Some(previous) = self.contents.take() {
            self.detach_child(self.root, previous);
        }
        let contents_id = self
            .new_block_with_parent(figure, self.root)
            .expect("FigureTree root must exist");
        self.contents = Some(contents_id);
        self.invalidate();
        contents_id
    }

    /// 获取内容块
    pub fn get_contents(&self) -> Option<FigureId> {
        self.contents
    }

    /// 添加子块到指定父块
    ///
    /// 对应 draw2d: parent.addChild(child) (不触发 revalidate)
    ///
    /// 与 `add_child()` 的区别：此方法不触发 revalidate()，用于批量构建场景。
    pub(crate) fn add_child_to(
        &mut self,
        parent_id: FigureId,
        figure: Box<dyn super::Figure>,
    ) -> FigureId {
        self.try_add_child_to(parent_id, figure)
            .unwrap_or_else(|_| FigureId::null())
    }

    /// 尝试添加子块到指定父块。
    ///
    /// parent 不存在或深度超限时不分配节点、不修改 UUID 映射，并返回错误。
    pub(crate) fn try_add_child_to(
        &mut self,
        parent_id: FigureId,
        figure: Box<dyn super::Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        self.new_block_with_parent(figure, parent_id)
    }

    /// 添加子块到指定父块，并设置子块的位置和尺寸
    ///
    /// # 坐标语义
    ///
    /// - bounds 位于 parent content domain
    /// - 添加后，子节点的 bounds 保持不变
    /// - 平移操作只修改当前节点，后代通过父链变换改变 surface 投影
    ///
    /// # 示例
    ///
    /// ```
    /// use novadraw_core::Color;
    /// use novadraw_scene::{figure::RectangleFigure, FigureTree};
    ///
    /// let mut scene = FigureTree::new();
    /// let parent_id = scene.builder().set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    /// let color = Color::hex("#3498db");
    /// // 添加子节点，bounds 位于 parent content domain
    /// let _child_id = scene.builder().add_child_with_bounds(parent_id, 10.0, 10.0, 50.0, 50.0, color);
    /// ```
    pub(crate) fn add_child_with_bounds(
        &mut self,
        parent_id: FigureId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: novadraw_core::Color,
    ) -> FigureId {
        let figure = super::figure::RectangleFigure::new_with_color(x, y, width, height, color);
        self.try_add_child_to(parent_id, Box::new(figure))
            .unwrap_or_else(|_| FigureId::null())
    }

    /// 添加子块
    ///
    /// 参考 draw2d: parent.addChild(child) -> revalidate()
    /// 与 `add_child_to()` 的区别：此方法会标记父容器需要重新布局，
    /// 并将父容器区域加入脏区域，下次 `perform_update()` 时会验证布局。
    ///
    /// # 使用场景
    ///
    /// 用于交互式修改（如拖拽添加、动态插入节点），不适合批量构建场景。
    /// 批量构建使用 `add_child_to()` 以避免不必要的更新触发。
    pub(crate) fn add_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        figure: Box<dyn super::Figure>,
    ) -> FigureId {
        let child_id = match self.try_add_child_to(parent_id, figure) {
            Ok(child_id) => child_id,
            Err(_) => return FigureId::null(),
        };
        let visual_bounds = self.blocks[child_id].visual_bounds();

        self.mark_invalid(update_manager, parent_id);
        update_manager.add_dirty_region(child_id, visual_bounds);
        self.mark_invalid(update_manager, child_id);

        child_id
    }

    pub(crate) fn apply_pending_mutations(
        &mut self,
        update_manager: &mut UpdateManager,
        mutations: Vec<PendingMutation>,
    ) -> bool {
        if mutations.is_empty() {
            return false;
        }

        let mut changed = false;
        for mutation in mutations {
            changed |= match mutation.into_kind() {
                kind @ PendingMutationKind::RemoveChild { .. } => {
                    self.apply_remove_mutation(update_manager, kind)
                }
                kind @ PendingMutationKind::Reparent { .. } => {
                    self.apply_reparent_mutation(update_manager, kind)
                }
                kind @ PendingMutationKind::AddChildFigure { .. } => {
                    self.apply_add_mutation(update_manager, kind)
                }
                PendingMutationKind::AddLayerFigure { .. }
                | PendingMutationKind::RemoveLayer { .. }
                | PendingMutationKind::MoveLayer { .. }
                | PendingMutationKind::ReparentLayer { .. }
                | PendingMutationKind::SetLayoutManager { .. }
                | PendingMutationKind::SetLayoutConstraint { .. }
                | PendingMutationKind::RemoveLayoutConstraint { .. }
                | PendingMutationKind::SetSizeOverride { .. }
                | PendingMutationKind::MoveChildToIndex { .. }
                | PendingMutationKind::BringChildToFront { .. }
                | PendingMutationKind::SendChildToBack { .. }
                | PendingMutationKind::SetChildClippingStrategy { .. } => false,
            };
        }

        changed
    }

    /// Removes a direct child through the update transaction.
    pub(crate) fn remove_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent: FigureId,
        child: FigureId,
    ) -> bool {
        self.apply_remove_mutation(
            update_manager,
            PendingMutationKind::RemoveChild { parent, child },
        )
    }

    /// Reparents a block through the update transaction.
    pub(crate) fn reparent(
        &mut self,
        update_manager: &mut UpdateManager,
        child: FigureId,
        new_parent: FigureId,
    ) -> bool {
        self.apply_reparent_mutation(
            update_manager,
            PendingMutationKind::Reparent { child, new_parent },
        )
    }

    /// 创建带父块的块
    fn new_block_with_parent(
        &mut self,
        figure: Box<dyn super::Figure>,
        parent_id: FigureId,
    ) -> Result<FigureId, GraphMutationError> {
        self.new_block_with_parent_admission(figure, parent_id, false)
    }

    fn new_block_with_parent_admission(
        &mut self,
        figure: Box<dyn super::Figure>,
        parent_id: FigureId,
        layer_admission: bool,
    ) -> Result<FigureId, GraphMutationError> {
        let bounds = figure.initial_bounds();
        let insets = figure.initial_insets();
        let style = figure.initial_style();
        let is_focusable = figure.initial_focusable();
        let is_focus_traversable = figure.initial_focus_traversable();
        let layout = LayoutState::for_figure(figure.as_ref());
        let parent_depth = self
            .blocks
            .get(parent_id)
            .map(|parent| parent.depth)
            .ok_or(GraphMutationError::ParentNotFound)?;
        let parent = &self.blocks[parent_id];
        match parent.child_policy() {
            ChildPolicy::Single if !parent.children.is_empty() => {
                return Err(GraphMutationError::ChildLimitExceeded { limit: 1 });
            }
            ChildPolicy::Layered if !layer_admission => {
                return Err(GraphMutationError::LayerKeyRequired);
            }
            ChildPolicy::Layered if figure.layer().is_none() => {
                return Err(GraphMutationError::LayerChildRequired);
            }
            _ => {}
        }
        let depth = parent_depth
            .checked_add(1)
            .filter(|depth| *depth <= MAX_TREE_DEPTH)
            .ok_or(GraphMutationError::DepthLimitExceeded {
                limit: MAX_TREE_DEPTH,
            })?;

        let uuid = Uuid::new_v4();
        let id = self.blocks.insert_with_key(|key| FigureNode {
            id: key,
            uuid,
            children: Vec::new(),
            parent: Some(parent_id),
            depth,
            figure,
            layout,
            state: NodeState {
                bounds,
                insets,
                is_focusable,
                is_focus_traversable,
                style,
                ..NodeState::default()
            },
        });
        self.uuid_map.insert(uuid, id);
        self.blocks[parent_id].children.push(id);
        if let Some(lifecycle) = self.blocks[id].figure.lifecycle() {
            lifecycle.on_attached(parent_id);
        }
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Added,
            block_id: id,
            parent_id,
        });
        self.mark_validation_path_invalid(parent_id);
        Ok(id)
    }

    pub(crate) fn add_layer_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        figure: Box<dyn super::Figure>,
    ) -> Result<FigureId, GraphMutationError> {
        let child_id = self.new_block_with_parent_admission(figure, parent_id, true)?;
        let visual_bounds = self.blocks[child_id].visual_bounds();
        self.mark_invalid(update_manager, parent_id);
        update_manager.add_dirty_region(child_id, visual_bounds);
        self.mark_invalid(update_manager, child_id);
        Ok(child_id)
    }

    fn attach_child_checked(
        &mut self,
        parent_id: FigureId,
        child_id: FigureId,
    ) -> Result<(), GraphMutationError> {
        self.attach_child_checked_admission(parent_id, child_id, false)
    }

    fn attach_child_checked_admission(
        &mut self,
        parent_id: FigureId,
        child_id: FigureId,
        layer_admission: bool,
    ) -> Result<(), GraphMutationError> {
        let new_depth = self.validate_attachment_admission(parent_id, child_id, layer_admission)?;

        self.blocks[parent_id].children.push(child_id);
        {
            let child = &mut self.blocks[child_id];
            child.parent = Some(parent_id);
            child.is_valid = false;
            if let Some(lifecycle) = child.figure.lifecycle() {
                lifecycle.on_attached(parent_id);
            }
        }
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Added,
            block_id: child_id,
            parent_id,
        });
        self.set_subtree_depth(child_id, new_depth);
        self.mark_validation_path_invalid(parent_id);
        Ok(())
    }

    fn detach_child(&mut self, parent_id: FigureId, child_id: FigureId) -> bool {
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };

        let old_len = parent.children.len();
        parent.children.retain(|&id| id != child_id);
        if parent.children.len() == old_len {
            return false;
        }
        parent.layout.constraints.remove(&child_id);

        if let Some(child) = self.blocks.get_mut(child_id) {
            if let Some(lifecycle) = child.figure.lifecycle() {
                lifecycle.on_detached(parent_id);
            }
            child.parent = None;
            child.is_valid = false;
        }
        self.emit_ancestor_event(AncestorEvent {
            kind: AncestorEventKind::Removed,
            block_id: child_id,
            parent_id,
        });
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ChildRemoved,
            container_id: parent_id,
            child_id: Some(child_id),
        });
        self.set_subtree_depth(child_id, 0);
        self.mark_validation_path_invalid(parent_id);
        true
    }

    fn validate_attachment(
        &self,
        parent_id: FigureId,
        child_id: FigureId,
    ) -> Result<usize, GraphMutationError> {
        self.validate_attachment_admission(parent_id, child_id, false)
    }

    fn validate_attachment_admission(
        &self,
        parent_id: FigureId,
        child_id: FigureId,
        layer_admission: bool,
    ) -> Result<usize, GraphMutationError> {
        let parent = self
            .blocks
            .get(parent_id)
            .ok_or(GraphMutationError::ParentNotFound)?;
        if !self.blocks.contains_key(child_id) {
            return Err(GraphMutationError::ChildNotFound);
        }

        if parent_id == child_id || self.is_descendant_or_self(parent_id, child_id) {
            return Err(GraphMutationError::CycleDetected);
        }
        if parent.children.contains(&child_id) {
            return Err(GraphMutationError::DuplicateChild);
        }
        match parent.child_policy() {
            ChildPolicy::Single if !parent.children.is_empty() => {
                return Err(GraphMutationError::ChildLimitExceeded { limit: 1 });
            }
            ChildPolicy::Layered if !layer_admission => {
                return Err(GraphMutationError::LayerKeyRequired);
            }
            ChildPolicy::Layered if self.blocks[child_id].figure.layer().is_none() => {
                return Err(GraphMutationError::LayerChildRequired);
            }
            _ => {}
        }

        let new_depth =
            parent
                .depth
                .checked_add(1)
                .ok_or(GraphMutationError::DepthLimitExceeded {
                    limit: MAX_TREE_DEPTH,
                })?;
        let subtree_height = self.subtree_height(child_id);
        if new_depth
            .checked_add(subtree_height)
            .is_none_or(|depth| depth > MAX_TREE_DEPTH)
        {
            return Err(GraphMutationError::DepthLimitExceeded {
                limit: MAX_TREE_DEPTH,
            });
        }

        Ok(new_depth)
    }

    fn subtree_height(&self, root_id: FigureId) -> usize {
        let Some(root) = self.blocks.get(root_id) else {
            return 0;
        };
        let root_depth = root.depth;
        let mut max_depth = root_depth;
        let mut stack = vec![root_id];
        while let Some(id) = stack.pop() {
            let Some(block) = self.blocks.get(id) else {
                continue;
            };
            max_depth = max_depth.max(block.depth);
            stack.extend(block.children.iter().copied());
        }
        max_depth.saturating_sub(root_depth)
    }

    fn set_subtree_depth(&mut self, root_id: FigureId, root_depth: usize) {
        let mut stack = vec![(root_id, root_depth)];
        while let Some((id, depth)) = stack.pop() {
            let children = match self.blocks.get_mut(id) {
                Some(block) => {
                    block.depth = depth;
                    block.children.clone()
                }
                None => continue,
            };
            stack.extend(children.into_iter().map(|child| (child, depth + 1)));
        }
    }

    fn contains_direct_child(&self, parent_id: FigureId, child_id: FigureId) -> bool {
        self.blocks
            .get(parent_id)
            .is_some_and(|parent| parent.children.contains(&child_id))
    }

    fn is_descendant_or_self(&self, mut node: FigureId, ancestor: FigureId) -> bool {
        for _ in 0..self.blocks.len() {
            if node == ancestor {
                return true;
            }
            let Some(parent) = self.blocks.get(node).and_then(|block| block.parent) else {
                return false;
            };
            node = parent;
        }
        false
    }

    fn apply_remove_mutation(
        &mut self,
        update_manager: &mut UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::RemoveChild { parent, child } = mutation else {
            return false;
        };
        if !self.blocks.contains_key(child) {
            return false;
        }
        if self
            .blocks
            .get(parent)
            .is_some_and(|node| node.child_policy() == ChildPolicy::Layered)
        {
            return false;
        }

        self.remove_child_internal(update_manager, parent, child)
    }

    pub(crate) fn remove_layer_child(
        &mut self,
        update_manager: &mut UpdateManager,
        parent: FigureId,
        child: FigureId,
    ) -> bool {
        if self
            .blocks
            .get(parent)
            .is_none_or(|node| node.child_policy() != ChildPolicy::Layered)
        {
            return false;
        }
        self.remove_child_internal(update_manager, parent, child)
    }

    fn remove_child_internal(
        &mut self,
        update_manager: &mut UpdateManager,
        parent: FigureId,
        child: FigureId,
    ) -> bool {
        if !self.blocks.contains_key(child) || !self.detach_child(parent, child) {
            return false;
        }

        if self.contents == Some(child) {
            self.contents = None;
        }

        self.mark_invalid(update_manager, parent);
        self.repaint(update_manager, parent, None);
        true
    }

    fn apply_reparent_mutation(
        &mut self,
        update_manager: &mut UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::Reparent { child, new_parent } = mutation else {
            return false;
        };
        let old_parent = self.blocks.get(child).and_then(|block| block.parent);
        if old_parent.is_none() {
            return false;
        }
        if old_parent == Some(new_parent) {
            return false;
        }

        let Some(visual_bounds) = self.blocks.get(child).map(FigureNode::visual_bounds) else {
            return false;
        };
        let Some(old_parent) = old_parent else {
            return false;
        };
        if self.blocks[old_parent].child_policy() == ChildPolicy::Layered {
            return false;
        }
        if !self.contains_direct_child(old_parent, child)
            || self.contains_direct_child(new_parent, child)
        {
            return false;
        }
        if self.validate_attachment(new_parent, child).is_err() {
            return false;
        }

        self.detach_child(old_parent, child);
        self.mark_invalid(update_manager, old_parent);
        self.repaint(update_manager, old_parent, None);

        if self.attach_child_checked(new_parent, child).is_err() {
            return false;
        }

        self.mark_invalid(update_manager, new_parent);
        update_manager.add_dirty_region(child, visual_bounds);
        self.repaint(update_manager, new_parent, None);
        true
    }

    pub(crate) fn reparent_layer_child(
        &mut self,
        update_manager: &mut UpdateManager,
        child: FigureId,
        new_parent: FigureId,
    ) -> bool {
        let Some(old_parent) = self.blocks.get(child).and_then(|block| block.parent) else {
            return false;
        };
        if old_parent == new_parent
            || !self.contains_direct_child(old_parent, child)
            || self.contains_direct_child(new_parent, child)
            || self
                .validate_attachment_admission(new_parent, child, true)
                .is_err()
        {
            return false;
        }
        let visual_bounds = self.blocks[child].visual_bounds();
        self.detach_child(old_parent, child);
        self.mark_invalid(update_manager, old_parent);
        self.repaint(update_manager, old_parent, None);
        if self
            .attach_child_checked_admission(new_parent, child, true)
            .is_err()
        {
            return false;
        }
        self.mark_invalid(update_manager, new_parent);
        update_manager.add_dirty_region(child, visual_bounds);
        self.repaint(update_manager, new_parent, None);
        true
    }

    fn apply_add_mutation(
        &mut self,
        update_manager: &mut UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::AddChildFigure { parent, figure } = mutation else {
            return false;
        };
        let Ok(child) = self.new_block_with_parent(figure, parent) else {
            return false;
        };
        let visual_bounds = self.blocks[child].visual_bounds();

        self.mark_invalid(update_manager, parent);
        self.mark_invalid(update_manager, child);
        update_manager.add_dirty_region(child, visual_bounds);
        self.repaint(update_manager, parent, None);
        true
    }

    /// 使布局失效，下次渲染时将重新计算布局
    ///
    /// 对应 draw2d: Figure.invalidate()
    pub fn invalidate(&mut self) {
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
    /// * `block_id` - 需要重新布局的块 ID
    pub fn mark_invalid(&mut self, update_manager: &mut UpdateManager, block_id: FigureId) {
        self.mark_validation_path_invalid(block_id);
        update_manager.add_invalid_figure(block_id);
    }

    /// 请求重绘指定块
    ///
    /// 对应 draw2d: Figure.repaint() -> UpdateManager.addDirtyRegion()
    /// 将块添加到更新管理器的脏区域队列中。
    ///
    /// # Arguments
    ///
    /// * `block_id` - 需要重绘的块 ID
    /// * `rect` - node-local 脏区域；`None` 表示完整 local border box
    pub fn repaint(
        &mut self,
        update_manager: &mut UpdateManager,
        block_id: FigureId,
        rect: Option<Rectangle>,
    ) {
        if let Some(block) = self.blocks.get(block_id) {
            if !self.is_effectively_visible(block_id) {
                return;
            }

            let dirty_rect = rect.unwrap_or_else(|| block.visual_bounds());
            update_manager.add_dirty_region(block_id, dirty_rect);
        }
    }

    /// 请求重绘整个场景
    ///
    /// 对应 draw2d: Figure.repaint() 使用整个 bounds
    pub fn repaint_all(&mut self, update_manager: &mut UpdateManager) {
        if let Some(contents_id) = self.contents {
            self.repaint(update_manager, contents_id, None);
        }
    }

    pub fn freeform_extent(&self, block_id: FigureId) -> Result<Rectangle, FreeformError> {
        let block = self
            .blocks
            .get(block_id)
            .ok_or(FreeformError::UnknownFigure(block_id))?;
        let state = block
            .layout
            .freeform
            .as_ref()
            .ok_or(FreeformError::NotFreeform(block_id))?;
        if state.extent_generation.is_none() {
            return Err(FreeformError::Unvalidated(block_id));
        }
        Ok(state.cached_extent)
    }

    fn recompute_freeform_extent(&mut self, block_id: FigureId) -> Result<(), LayoutError> {
        let dirty = self
            .blocks
            .get(block_id)
            .and_then(|block| block.layout.freeform.as_ref())
            .is_some_and(|state| state.dirty);
        if !dirty {
            return Ok(());
        }

        let children = self.blocks[block_id].children.clone();
        let mut extent: Option<Rectangle> = None;
        for child_id in children {
            if self.blocks[child_id].layout.freeform.is_some() {
                self.recompute_freeform_extent(child_id)?;
            }
            let child = &self.blocks[child_id];
            let contribution = if let Some(state) = child.layout.freeform.as_ref() {
                let child_extent = state.cached_extent;
                let bounds = child.figure_bounds();
                let transform = Affine2D::from_translation(bounds.x, bounds.y)
                    * child.child_transform().affine();
                transform_rectangle(transform, child_extent)
                    .ok_or(LayoutError::NonFiniteGeometry { figure: child_id })?
            } else {
                child.figure_bounds()
            };
            if !finite_rectangle(contribution) {
                return Err(LayoutError::NonFiniteGeometry { figure: child_id });
            }
            extent = Some(match extent {
                Some(current) => current.union(contribution),
                None => contribution,
            });
        }

        let new_extent = extent.unwrap_or(Rectangle::ZERO);
        let (old_extent, changed) = {
            let block = &mut self.blocks[block_id];
            let generation = block.layout.generation();
            let state = block
                .layout
                .freeform
                .as_mut()
                .expect("dirty freeform state must exist");
            let old_extent = state.cached_extent;
            state.cached_extent = new_extent;
            state.extent_generation = Some(generation);
            state.dirty = false;
            (old_extent, old_extent != new_extent)
        };
        if changed {
            self.record_property_change(
                block_id,
                FREEFORM_EXTENT_PROPERTY,
                PropertyValue::Rectangle(old_extent),
                PropertyValue::Rectangle(new_extent),
            );
        }
        Ok(())
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
    pub fn perform_update(&mut self, update_manager: &mut UpdateManager) -> NdCanvas {
        let mut canvas = NdCanvas::new();
        update_manager.perform_update(self, &mut canvas);
        canvas
    }

    /// 执行 validation phase 的图级语义。
    ///
    /// UpdateManager 只提供待验证队列与 phase 触发，
    /// FigureTree 自身决定哪些节点可参与验证以及如何 revalidate。
    pub fn perform_validation_cycle(
        &mut self,
        update_manager: &mut UpdateManager,
    ) -> Result<(), ValidationError> {
        self.perform_validation_cycle_with_budget(update_manager, DEFAULT_VALIDATION_BUDGET)
    }

    pub fn perform_validation_cycle_with_budget(
        &mut self,
        update_manager: &mut UpdateManager,
        budget: usize,
    ) -> Result<(), ValidationError> {
        let mut processed = 0;
        let mut invalidation_chain = Vec::new();
        loop {
            let block_ids = update_manager.drain_invalid_blocks();
            if block_ids.is_empty() {
                return Ok(());
            }
            let remaining = budget.saturating_sub(processed);
            if block_ids.len() > remaining {
                invalidation_chain.extend(block_ids.iter().take(remaining).copied());
                for block_id in &block_ids {
                    update_manager.add_invalid_figure(*block_id);
                }
                return Err(ValidationError::NonConvergingValidation {
                    budget,
                    invalidation_chain,
                });
            }
            processed += block_ids.len();
            invalidation_chain.extend(block_ids.iter().copied());

            for block_id in &block_ids {
                self.mark_validation_path_invalid(*block_id);
            }

            let mut validation_roots: Vec<FigureId> = block_ids
                .into_iter()
                .filter_map(|block_id| self.validation_root(block_id))
                .collect();
            validation_roots.sort_by_key(|id| self.block_depth(*id).unwrap_or(usize::MAX));
            validation_roots.dedup();

            for root_id in validation_roots {
                if let Err(error) = self.revalidate_with_update(update_manager, root_id) {
                    update_manager.add_invalid_figure(root_id);
                    return Err(error.into());
                }
            }
        }
    }

    fn validation_root(&self, block_id: FigureId) -> Option<FigureId> {
        let mut current = block_id;
        let mut root = block_id;
        loop {
            let block = self.blocks.get(current)?;
            let Some(parent_id) = block.parent else {
                return Some(root);
            };
            let parent = self.blocks.get(parent_id)?;
            if parent.is_valid {
                return Some(root);
            }
            root = parent_id;
            current = parent_id;
        }
    }

    /// 重新验证布局（递归），如果布局无效则重新计算
    ///
    /// 从指定容器开始，递归执行布局。
    /// 只有设置了布局管理器的容器才会执行布局。
    /// 参考 draw2d: Figure.layout() { if (layoutManager != null) layoutManager.layout() }
    fn revalidate_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
    ) -> Result<(), LayoutError> {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return Ok(());
        }
        if !self.is_effectively_visible(container_id) {
            return Ok(());
        }

        let prevalidate_children = self.blocks[container_id]
            .layout
            .manager
            .as_deref()
            .is_some_and(LayoutManager::requires_valid_children_before_layout);
        if prevalidate_children {
            self.revalidate_children_with_update(update_manager, container_id)?;
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|b| b.layout.manager.take());

        if let Some(mut layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let mut output = LayoutOutput::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let snapshot = LayoutSnapshot::new(self);
                layout_manager.layout(container_id, &snapshot, &mut output)
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            let layout_result = match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            };
            layout_result?;
            self.apply_layout_output(update_manager, container_id, output)?;
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        self.revalidate_children_with_update(update_manager, container_id)?;
        self.recompute_freeform_extent(container_id)?;
        if let Some(block) = self.blocks.get_mut(container_id) {
            let bounds = block.figure_bounds();
            if let Some(lifecycle) = block.figure.lifecycle() {
                lifecycle.validate(bounds);
            }
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    /// 递归验证子容器的布局
    fn revalidate_children_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
    ) -> Result<(), LayoutError> {
        // 先收集子元素 ID，避免在迭代过程中同时持有不可变和可变引用
        let children: Vec<FigureId> = self
            .blocks
            .get(parent_id)
            .map(|b| b.children.clone())
            .unwrap_or_default();

        for child_id in children {
            self.revalidate_with_update(update_manager, child_id)?;
        }
        Ok(())
    }

    /// Immediately validates a subtree while preserving UpdateManager damage.
    ///
    /// This is the Draw2D `Figure.validate()` equivalent used by coordinated
    /// operations such as `ZoomManager`: scale invalidation is resolved before
    /// the new viewport location is applied.
    pub fn validate_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
    ) -> Result<(), LayoutError> {
        self.revalidate_with_update(update_manager, container_id)
    }

    /// 立即验证指定子树，不产生 damage。
    ///
    /// 该入口用于初始场景构建；运行时更新应通过 `mark_invalid` 和
    /// `UpdateManager::perform_update` 执行完整事务。
    pub fn revalidate(&mut self, container_id: FigureId) {
        self.try_revalidate(container_id)
            .expect("layout validation failed");
    }

    pub fn try_revalidate(&mut self, container_id: FigureId) -> Result<(), LayoutError> {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return Ok(());
        }
        if !self.is_effectively_visible(container_id) {
            return Ok(());
        }

        let prevalidate_children = self.blocks[container_id]
            .layout
            .manager
            .as_deref()
            .is_some_and(LayoutManager::requires_valid_children_before_layout);
        if prevalidate_children {
            let children = self.blocks[container_id].children.clone();
            for child_id in children {
                self.try_revalidate(child_id)?;
            }
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|block| block.layout.manager.take());
        if let Some(mut layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let mut output = LayoutOutput::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let snapshot = LayoutSnapshot::new(self);
                layout_manager.layout(container_id, &snapshot, &mut output)
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            let layout_result = match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            };
            layout_result?;
            self.apply_layout_output_without_update(container_id, output)?;
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        let children = self
            .blocks
            .get(container_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        for child_id in children {
            self.try_revalidate(child_id)?;
        }
        self.recompute_freeform_extent(container_id)?;
        if let Some(block) = self.blocks.get_mut(container_id) {
            let bounds = block.figure_bounds();
            if let Some(lifecycle) = block.figure.lifecycle() {
                lifecycle.validate(bounds);
            }
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    fn validate_layout_output(
        &self,
        container_id: FigureId,
        output: &LayoutOutput,
    ) -> Result<(), LayoutError> {
        for change in &output.changes {
            let child_id = match change {
                LayoutChange::Bounds(child_id, _)
                | LayoutChange::Visibility(child_id, _)
                | LayoutChange::Invalidate(child_id) => Some(*child_id),
                LayoutChange::Property { figure, .. }
                | LayoutChange::CoordinateSystemChanged(figure)
                | LayoutChange::Repaint(figure)
                | LayoutChange::RepaintParent(figure) => {
                    if *figure != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: *figure,
                        });
                    }
                    None
                }
            };
            if let Some(child_id) = child_id
                && self
                    .blocks
                    .get(child_id)
                    .is_none_or(|child| child.parent != Some(container_id))
            {
                return Err(LayoutError::InvalidChild {
                    container: container_id,
                    child: child_id,
                });
            }
        }
        Ok(())
    }

    fn apply_layout_output(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        output: LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.validate_layout_output(container_id, &output)?;
        for change in output.changes {
            match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    self.set_bounds_with_update(
                        update_manager,
                        child_id,
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                    );
                }
                LayoutChange::Visibility(child_id, visible) => {
                    self.set_visible_with_update(update_manager, child_id, visible);
                }
                LayoutChange::Invalidate(child_id) => {
                    self.mark_invalid(update_manager, child_id);
                }
                LayoutChange::Property {
                    figure,
                    property,
                    old_value,
                    new_value,
                } => {
                    self.record_property_change(figure, property, old_value, new_value);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(figure) => {
                    self.repaint(update_manager, figure, None);
                }
                LayoutChange::RepaintParent(figure) => {
                    if let Some(parent) = self.parent_id(figure) {
                        self.repaint(update_manager, parent, None);
                    }
                }
            }
        }
        Ok(())
    }

    fn apply_layout_output_without_update(
        &mut self,
        container_id: FigureId,
        output: LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.validate_layout_output(container_id, &output)?;
        for change in output.changes {
            match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    let old_bounds = self.figure_bounds(child_id);
                    self.set_bounds(child_id, bounds.x, bounds.y, bounds.width, bounds.height);
                    if old_bounds
                        .is_some_and(|old| old.width != bounds.width || old.height != bounds.height)
                        && let Some(child) = self.blocks.get_mut(child_id)
                    {
                        child.is_valid = false;
                    }
                }
                LayoutChange::Visibility(child_id, visible) => {
                    self.set_visible(child_id, visible);
                }
                LayoutChange::Invalidate(child_id) => {
                    self.mark_validation_path_invalid(child_id);
                }
                LayoutChange::Property {
                    figure,
                    property,
                    old_value,
                    new_value,
                } => {
                    self.record_property_change(figure, property, old_value, new_value);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(_) | LayoutChange::RepaintParent(_) => {}
            }
        }
        Ok(())
    }

    /// 获取子元素 ID 列表
    #[allow(dead_code)]
    fn get_children_ids(&self, parent_id: FigureId) -> Vec<FigureId> {
        self.blocks
            .get(parent_id)
            .map(|b| b.children.clone())
            .unwrap_or_default()
    }

    /// 检查布局是否有效
    pub fn is_layout_valid(&self) -> bool {
        self.blocks
            .get(self.contents.unwrap_or(self.root))
            .map(|block| block.is_valid)
            .unwrap_or(true)
    }

    /// 返回单个节点的 validation 状态。
    pub fn is_valid(&self, block_id: FigureId) -> bool {
        self.blocks
            .get(block_id)
            .is_some_and(|block| block.is_valid)
    }

    /// 计算节点首选尺寸。显式覆盖优先，其次委托容器 LayoutManager，最后回退到 Figure。
    pub fn preferred_size(
        &self,
        block_id: FigureId,
        w_hint: f64,
        h_hint: f64,
    ) -> Option<(f64, f64)> {
        let block = self.blocks.get(block_id)?;
        let (w_hint, h_hint) = block.layout_size_hints(w_hint, h_hint);
        if let Some(size) = block.preferred_size {
            return Some(block.project_preferred_size(size));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let generation = block.layout.generation();
            if let Some(cached) = block.layout.cache.borrow().preferred
                && cached.matches(generation, w_hint, h_hint)
            {
                return Some(cached.size);
            }
            let snapshot = LayoutSnapshot::new(self);
            let size = block.project_preferred_size(
                layout.get_preferred_size(block_id, w_hint, h_hint, &snapshot),
            );
            block.layout.cache.borrow_mut().preferred = Some(CachedMeasurement {
                generation,
                w_hint,
                h_hint,
                size,
            });
            return Some(size);
        }
        Some(owner_scoped_border_size(
            block.figure.intrinsic_size(),
            block.border_snapshot.as_ref(),
        ))
    }

    /// 计算节点最小尺寸。显式覆盖优先，其次委托容器 LayoutManager，最后回退到 Figure。
    pub fn minimum_size(&self, block_id: FigureId, w_hint: f64, h_hint: f64) -> Option<(f64, f64)> {
        let block = self.blocks.get(block_id)?;
        let (w_hint, h_hint) = block.layout_size_hints(w_hint, h_hint);
        if let Some(size) = block.minimum_size {
            return Some(block.project_minimum_size(size));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let generation = block.layout.generation();
            if let Some(cached) = block.layout.cache.borrow().minimum
                && cached.matches(generation, w_hint, h_hint)
            {
                return Some(cached.size);
            }
            let snapshot = LayoutSnapshot::new(self);
            let size = block
                .project_minimum_size(layout.get_minimum_size(block_id, w_hint, h_hint, &snapshot));
            block.layout.cache.borrow_mut().minimum = Some(CachedMeasurement {
                generation,
                w_hint,
                h_hint,
                size,
            });
            return Some(size);
        }
        Some(owner_scoped_border_size(
            block.figure.intrinsic_minimum_size(),
            block.border_snapshot.as_ref(),
        ))
    }

    /// 返回节点最大尺寸。显式覆盖优先，否则回退到 Figure。
    pub fn maximum_size(&self, block_id: FigureId) -> Option<(f64, f64)> {
        let block = self.blocks.get(block_id)?;
        Some(block.maximum_size.unwrap_or((f64::INFINITY, f64::INFINITY)))
    }

    pub fn set_preferred_size(&mut self, block_id: FigureId, size: Option<(f64, f64)>) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.preferred_size == size {
            return false;
        }
        block.preferred_size = size;
        self.mark_validation_path_invalid_for(block_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub fn set_minimum_size(&mut self, block_id: FigureId, size: Option<(f64, f64)>) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.minimum_size == size {
            return false;
        }
        block.minimum_size = size;
        self.mark_validation_path_invalid_for(block_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub fn set_maximum_size(&mut self, block_id: FigureId, size: Option<(f64, f64)>) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.maximum_size == size {
            return false;
        }
        block.maximum_size = size;
        self.mark_validation_path_invalid_for(block_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn set_child_clipping_strategy(
        &mut self,
        block_id: FigureId,
        strategy: ChildClippingStrategy,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.child_clipping_strategy() == strategy {
            return false;
        }
        block.state.child_clipping_strategy = Some(strategy);
        self.notify_block_changed(block_id);
        true
    }

    pub fn child_clipping_strategy(&self, block_id: FigureId) -> Option<ChildClippingStrategy> {
        self.blocks
            .get(block_id)
            .map(FigureNode::child_clipping_strategy)
    }

    /// 渲染场景图
    ///
    /// 使用递归实现 Figure 树的渲染遍历。
    /// 渲染顺序（参考 draw2d）：
    /// 1. paintFigure() - 绘制自身
    /// 2. paintClientArea() - 绘制子元素
    /// 3. paintBorder() - 绘制边框
    pub fn render(&self) -> NdCanvas {
        let mut gc = NdCanvas::new();
        gc.damage_mut().set_full();
        self.render_to(&mut gc);
        gc
    }

    /// 渲染到上下文（递归实现）
    pub(crate) fn render_to(&self, gc: &mut NdCanvas) {
        let start_id = self.contents.unwrap_or(self.root);
        let scene_ref = FigureTreeRenderRef {
            blocks: &self.blocks,
        };
        let mut renderer = FigureRenderer::new(&scene_ref, gc);
        renderer.render(start_id);
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
    pub fn print_tree(&self) {
        eprintln!("\n========== 场景图结构 ==========");
        self.print_block(self.root, 0);
        eprintln!("=================================\n");
    }

    /// 递归打印单个块（内部使用）
    #[cfg(feature = "debug_render")]
    fn print_block(&self, block_id: FigureId, depth: usize) {
        let indent = "  ".repeat(depth);
        if let Some(block) = self.blocks.get(block_id) {
            let bounds = block.figure_bounds();
            let visibility = if block.is_visible { "V" } else { "H" };
            eprintln!(
                "{}{} {:?}: {} bounds=({:.0},{:.0},{:.0},{:.0})",
                indent,
                visibility,
                block_id,
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
    #[allow(clippy::collapsible_if)]
    pub fn print_render_order(&self) {
        let start_id = self.contents.unwrap_or(self.root);
        let mut stack = vec![start_id];

        eprintln!("\n========== 渲染顺序 ==========");
        let mut order = Vec::new();

        while let Some(block_id) = stack.pop() {
            if let Some(block) = self.blocks.get(block_id) {
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

    /// 获取块
    pub fn get_block(&self, id: FigureId) -> Option<&FigureNode> {
        self.blocks.get(id)
    }

    pub(crate) fn is_layered_pane(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|node| node.child_policy() == ChildPolicy::Layered)
    }

    pub(crate) fn is_layer_figure(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|node| node.figure.layer().is_some())
    }

    pub(crate) fn block(&self, id: FigureId) -> Option<&FigureNode> {
        self.blocks.get(id)
    }

    /// 返回指定父节点的 child 顺序。
    ///
    /// 顺序与 Draw2D 一致：数组靠前的 child 先绘制，靠后的 child 后绘制并位于更高 z-order。
    pub fn child_order(&self, parent_id: FigureId) -> Option<Vec<FigureId>> {
        self.blocks
            .get(parent_id)
            .map(|block| block.children.clone())
    }

    /// Returns the direct parent of a block.
    pub fn parent_id(&self, block_id: FigureId) -> Option<FigureId> {
        self.blocks.get(block_id).and_then(|block| block.parent)
    }

    /// Returns whether a node is currently attached to this tree's root.
    pub fn is_attached(&self, block_id: FigureId) -> bool {
        let mut current = Some(block_id);
        for _ in 0..=self.blocks.len() {
            let Some(id) = current else {
                return false;
            };
            if id == self.root {
                return true;
            }
            current = self.blocks.get(id).and_then(|block| block.parent);
        }
        false
    }

    /// 返回 child 在父节点内的 z-order index。
    ///
    /// index 越大表示越靠前绘制、越靠上层。
    pub fn child_z_index(&self, parent_id: FigureId, child_id: FigureId) -> Option<usize> {
        self.blocks
            .get(parent_id)?
            .children
            .iter()
            .position(|&id| id == child_id)
    }

    /// 将直接 child 移动到指定 z-order index。
    ///
    /// `index == 0` 表示最底层；`index == children.len() - 1` 表示最顶层。
    pub(crate) fn move_child_to_index(
        &mut self,
        parent_id: FigureId,
        child_id: FigureId,
        index: usize,
    ) -> bool {
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };

        let Some(old_index) = parent.children.iter().position(|&id| id == child_id) else {
            return false;
        };

        if index >= parent.children.len() || old_index == index {
            return false;
        }

        let child = parent.children.remove(old_index);
        parent.children.insert(index, child);
        self.notify_block_changed(parent_id);
        true
    }

    /// 将直接 child 移动到最高 z-order。
    pub(crate) fn bring_child_to_front(&mut self, parent_id: FigureId, child_id: FigureId) -> bool {
        let Some(last_index) = self
            .blocks
            .get(parent_id)
            .and_then(|parent| parent.children.len().checked_sub(1))
        else {
            return false;
        };
        self.move_child_to_index(parent_id, child_id, last_index)
    }

    /// 将直接 child 移动到最低 z-order。
    pub(crate) fn send_child_to_back(&mut self, parent_id: FigureId, child_id: FigureId) -> bool {
        self.move_child_to_index(parent_id, child_id, 0)
    }

    /// 获取指定块的 Figure bounds。
    pub fn figure_bounds(&self, id: FigureId) -> Option<Rectangle> {
        self.blocks.get(id).map(FigureNode::figure_bounds)
    }

    pub fn is_connection_figure(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|block| block.figure.connection().is_some())
    }

    pub fn connection_route_points(&self, id: FigureId) -> Option<&PointList> {
        self.blocks
            .get(id)?
            .figure
            .connection()
            .map(|connection| connection.route_points())
    }

    pub fn connection_stroke_color(&self, id: FigureId) -> Option<novadraw_core::Color> {
        self.blocks
            .get(id)?
            .figure
            .connection()
            .map(|connection| connection.connection_stroke_color())
    }

    pub fn point_list_points(&self, id: FigureId) -> Option<Vec<Vec2>> {
        let block = self.blocks.get(id)?;
        let bounds = block.figure_bounds();
        Some(
            block
                .figure
                .point_list()?
                .local_points()
                .iter()
                .map(|point| Vec2::new(point.x() + bounds.x, point.y() + bounds.y))
                .collect(),
        )
    }

    pub(crate) fn commit_point_list(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        parent_points: Vec<Vec2>,
    ) -> Result<bool, ShapeMutationError> {
        if parent_points
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(point_list) = block.figure.point_list() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old_points = self
            .point_list_points(id)
            .expect("validated point-list capability");
        if old_points == parent_points {
            return Ok(false);
        }
        let old_bounds = block.figure_bounds();
        let old_visual_bounds = block.visual_bounds();
        let parent_id = block.parent;
        let visible = self.is_effectively_visible(id);
        let (new_bounds, local_points) = normalize_points(
            parent_points.clone(),
            point_list.stroke_width(),
            point_list.painted_minimum(),
        );
        if !finite_rectangle(new_bounds) {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }

        if visible {
            self.erase(update_manager, id, old_bounds, old_visual_bounds, parent_id);
        }
        let block = self
            .blocks
            .get_mut(id)
            .ok_or(ShapeMutationError::UnknownFigure(id))?;
        block.set_node_bounds(new_bounds);
        block
            .figure
            .point_list_mut()
            .ok_or(ShapeMutationError::WrongCapability(id))?
            .commit_geometry(new_bounds, local_points);

        self.notify_block_changed(id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            block_id: id,
            old_bounds,
            new_bounds,
        });
        self.emit_property_event(PropertyChangeEvent {
            block_id: id,
            property: "points",
            old_value: PropertyValue::PointList(old_points),
            new_value: PropertyValue::PointList(parent_points),
        });
        self.mark_invalid(update_manager, id);
        self.mark_freeform_ancestor_extents_dirty(id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        Ok(true)
    }

    pub(crate) fn replace_border(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        border: Option<Arc<dyn Border>>,
    ) -> Result<bool, ShapeMutationError> {
        let metrics = border.as_deref().map(|border| {
            let insets = border.get_insets();
            let preferred = border.preferred_size();
            (insets, preferred)
        });
        if metrics.is_some_and(|(insets, preferred)| {
            [
                insets.0,
                insets.1,
                insets.2,
                insets.3,
                preferred.0,
                preferred.1,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.0)
        }) {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let (supports_border, same_border) =
            self.blocks.get_mut(id).map_or((false, false), |block| {
                match block.figure.bordered_mut() {
                    Some(bordered) => {
                        let same = match (bordered.border(), border.as_ref()) {
                            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
                            (None, None) => true,
                            _ => false,
                        };
                        (true, same)
                    }
                    None => (false, false),
                }
            });
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        if same_border {
            return Ok(false);
        }
        if !supports_border {
            return Err(ShapeMutationError::WrongCapability(id));
        }
        let old_bounds = block.figure_bounds();
        let old_visual_bounds = block.visual_bounds();
        let parent_id = block.parent;
        let visible = self.is_effectively_visible(id);
        let had_border = block.figure.get_border().is_some();

        if visible {
            self.erase(update_manager, id, old_bounds, old_visual_bounds, parent_id);
        }
        let has_border = {
            let block = self
                .blocks
                .get_mut(id)
                .ok_or(ShapeMutationError::UnknownFigure(id))?;
            block
                .figure
                .bordered_mut()
                .ok_or(ShapeMutationError::WrongCapability(id))?
                .replace_border(border);
            block.border_snapshot = None;
            block.insets = block
                .figure
                .get_border()
                .map(Border::get_insets)
                .unwrap_or_default();
            block.figure.get_border().is_some()
        };

        self.record_property_change(
            id,
            "border",
            PropertyValue::Bool(had_border),
            PropertyValue::Bool(has_border),
        );
        self.mark_invalid(update_manager, id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        Ok(true)
    }

    pub(crate) fn set_corner_dimensions_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        dimensions: Dimension,
    ) -> Result<bool, ShapeMutationError> {
        if !dimensions.width.is_finite() || !dimensions.height.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if dimensions.width < 0.0 || dimensions.height < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(rounded) = block
            .figure
            .as_ref()
            .as_any()
            .downcast_ref::<RoundedRectangleFigure>()
        else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = rounded.corner_dimensions();
        if old == dimensions {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<RoundedRectangleFigure>()
            .expect("validated rounded rectangle capability")
            .set_corner_dimensions(dimensions);
        self.record_property_change(
            id,
            "corner_dimensions",
            PropertyValue::Size(old),
            PropertyValue::Size(dimensions),
        );
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_triangle_direction_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        direction: Direction,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(triangle) = block
            .figure
            .as_ref()
            .as_any()
            .downcast_ref::<TriangleFigure>()
        else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = triangle.direction;
        if old == direction {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<TriangleFigure>()
            .expect("validated triangle capability")
            .set_direction(direction);
        self.record_property_change(
            id,
            "direction",
            PropertyValue::Text(format!("{old:?}")),
            PropertyValue::Text(format!("{direction:?}")),
        );
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn commit_connection_route(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        parent_points: &PointList,
    ) -> bool {
        let Some((stroke_width, old_bounds, old_visual_bounds, parent_id, visible)) =
            self.blocks.get(id).and_then(|block| {
                block.figure.connection().map(|connection| {
                    (
                        connection.connection_stroke_width(),
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
        let Some(point_bounds) = parent_points.bounds() else {
            return false;
        };
        let path_bounds = point_bounds.inflate(stroke_width / 2.0, stroke_width / 2.0);
        if !finite_rectangle(path_bounds) {
            return false;
        }
        let mut local_points = parent_points.clone();
        local_points.translate(-path_bounds.x, -path_bounds.y);

        if visible {
            self.erase(update_manager, id, old_bounds, old_visual_bounds, parent_id);
        }
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        block.set_node_bounds(path_bounds);
        let Some(connection) = block.figure.connection_mut() else {
            return false;
        };
        connection.commit_route_points(local_points);

        self.notify_block_changed(id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            block_id: id,
            old_bounds,
            new_bounds: path_bounds,
        });
        self.mark_invalid(update_manager, id);
        self.mark_freeform_ancestor_extents_dirty(id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        true
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
            self.erase(update_manager, id, old_bounds, old_visual_bounds, parent_id);
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
            block_id: id,
            old_bounds,
            new_bounds: Rectangle::ZERO,
        });
        self.mark_invalid(update_manager, id);
        self.mark_freeform_ancestor_extents_dirty(id);
        true
    }

    /// 返回节点从 FigureTree 根节点开始计算的深度。
    pub fn block_depth(&self, id: FigureId) -> Option<usize> {
        self.blocks.get(id).map(|block| block.depth)
    }

    /// 返回节点自身的本地可见性标志。
    pub fn is_visible(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .map(|block| block.is_visible)
            .unwrap_or(false)
    }

    /// 返回节点自身的本地启用标志。
    pub fn is_enabled(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .map(|block| block.is_enabled)
            .unwrap_or(false)
    }

    pub fn is_opaque(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| block.is_opaque)
    }

    pub fn insets(&self, id: FigureId) -> Option<(f64, f64, f64, f64)> {
        self.blocks.get(id).map(|block| block.insets)
    }

    pub fn border_is_effectively_opaque(&self, id: FigureId) -> Option<bool> {
        let block = self.blocks.get(id)?;
        let border = block.figure.get_border()?;
        Some(border.is_opaque() && self.resolved_style(id)?.alpha >= 1.0)
    }

    pub fn figure_style(&self, id: FigureId) -> Option<&FigureStyle> {
        self.blocks.get(id).map(|block| &block.style)
    }

    pub fn resolved_style(&self, id: FigureId) -> Option<ResolvedStyle> {
        self.blocks.get(id)?;
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.blocks.get(node_id)?;
            chain.push(node_id);
            current = node.parent;
        }
        let mut result = ResolvedStyle::default();
        for node_id in chain.into_iter().rev() {
            result.apply_override(&self.blocks[node_id].style);
        }
        Some(result)
    }

    pub(crate) fn refresh_label_layouts(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        resources: &crate::ResourceRegistry,
    ) -> Result<Vec<FigureId>, TextError> {
        let labels = self
            .blocks
            .iter()
            .filter_map(|(id, block)| block.figure.label().is_some().then_some(id))
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for id in labels {
            let style = self
                .resolved_style(id)
                .expect("attached label has resolved style");
            let font = novadraw_render::FontDescriptor::parse(&style.font)?;
            let bounds = self.blocks[id].client_area();
            let label = self.blocks[id]
                .figure
                .label_mut()
                .expect("label capability checked before mutable borrow");
            let icon = label.icon().and_then(|id| resources.image_ref(id));
            if label.refresh_layout(text, &font, bounds, icon)? {
                changed.push(id);
            }
        }
        Ok(changed)
    }

    pub(crate) fn refresh_image_figures(
        &mut self,
        resources: &crate::ResourceRegistry,
    ) -> Vec<FigureId> {
        let images = self
            .blocks
            .iter()
            .filter_map(|(id, block)| {
                block
                    .figure
                    .as_ref()
                    .as_any()
                    .is::<ImageFigure>()
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for id in images {
            let image = self.blocks[id]
                .figure
                .as_ref()
                .as_any()
                .downcast_ref::<ImageFigure>()
                .expect("image type checked before refresh")
                .image();
            let Ok(status) = resources.status(image.resource_id()) else {
                continue;
            };
            let image_ref = resources.image_ref(image);
            let figure = self.blocks[id]
                .figure
                .as_mut()
                .as_any_mut()
                .downcast_mut::<ImageFigure>()
                .expect("image type checked before refresh");
            if figure.refresh(status, image_ref) {
                changed.push(id);
            }
        }
        changed
    }

    pub(crate) fn label(&self, id: FigureId) -> Option<&LabelFigure> {
        self.blocks.get(id)?.figure.label()
    }

    pub(crate) fn image_figure(&self, id: FigureId) -> Option<&ImageFigure> {
        self.blocks.get(id)?.figure.as_ref().as_any().downcast_ref()
    }

    pub(crate) fn set_image_figure(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        image: crate::ImageId,
    ) -> Result<bool, ShapeMutationError> {
        let Some(previous_image) = self.image_figure(id).map(ImageFigure::image) else {
            return Err(if self.blocks.contains_key(id) {
                ShapeMutationError::WrongCapability(id)
            } else {
                ShapeMutationError::UnknownFigure(id)
            });
        };
        if previous_image == image {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<ImageFigure>()
            .expect("image type checked before mutation")
            .set_image(image);
        self.record_property_change(
            id,
            "image",
            PropertyValue::Text(format!("{previous_image:?}")),
            PropertyValue::Text(format!("{image:?}")),
        );
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_image_alignment(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        alignment: crate::Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let Some(previous) = self.image_figure(id).map(ImageFigure::alignment) else {
            return Err(if self.blocks.contains_key(id) {
                ShapeMutationError::WrongCapability(id)
            } else {
                ShapeMutationError::UnknownFigure(id)
            });
        };
        if previous == alignment {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<ImageFigure>()
            .expect("image type checked before mutation")
            .set_alignment(alignment);
        self.record_property_change(
            id,
            "image_alignment",
            PropertyValue::Text(format!("{previous:?}")),
            PropertyValue::Text(format!("{alignment:?}")),
        );
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mutate_label(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
        mutate: impl FnOnce(&mut LabelFigure),
        revalidate: bool,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        if block.figure.label().is_none() {
            return Err(ShapeMutationError::WrongCapability(id));
        }
        if old_value == new_value {
            return Ok(false);
        }
        let label = self.blocks[id]
            .figure
            .label_mut()
            .expect("label capability checked before mutation");
        mutate(label);
        self.record_property_change(id, property, old_value, new_value);
        if revalidate {
            self.mark_invalid(update_manager, id);
        }
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn clickable_snapshot(&self, id: FigureId) -> Option<ClickableSnapshot> {
        self.blocks
            .get(id)?
            .figure
            .clickable()
            .map(|clickable| clickable.clickable_model().snapshot())
    }

    pub(crate) fn clickable_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| block.figure.clickable().is_some().then_some(id))
            .collect()
    }

    pub(crate) fn activate_clickable(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
    ) -> bool {
        if !self.is_effectively_enabled(id) {
            return false;
        }
        let Some(clickable) = self
            .blocks
            .get_mut(id)
            .and_then(|block| block.figure.clickable_mut())
        else {
            return false;
        };
        let (selection_change, revision) =
            super::figure::widget::activate(clickable.clickable_model_mut());
        self.notify_block_changed(id);
        if let Some((old, new)) = selection_change {
            self.emit_property_event(PropertyChangeEvent {
                block_id: id,
                property: "selected",
                old_value: PropertyValue::Bool(old),
                new_value: PropertyValue::Bool(new),
            });
        }
        self.notification_effects.emit_action(ActionEvent {
            block_id: id,
            revision,
        });
        self.repaint(update_manager, id, None);
        true
    }

    pub(crate) fn set_clickable_selected(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        selected: bool,
    ) -> Result<bool, WidgetError> {
        let Some(block) = self.blocks.get_mut(id) else {
            return Err(WidgetError::UnknownFigure(id));
        };
        let Some(clickable) = block.figure.clickable_mut() else {
            return Err(WidgetError::WrongCapability(id));
        };
        let old = clickable.clickable_model().is_selected();
        if !super::figure::widget::set_selected(clickable.clickable_model_mut(), selected) {
            return Ok(false);
        }
        self.record_property_change(
            id,
            "selected",
            PropertyValue::Bool(old),
            PropertyValue::Bool(selected),
        );
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_clickable_rollover_enabled(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        enabled: bool,
    ) -> Result<bool, WidgetError> {
        let Some(block) = self.blocks.get_mut(id) else {
            return Err(WidgetError::UnknownFigure(id));
        };
        let Some(clickable) = block.figure.clickable_mut() else {
            return Err(WidgetError::WrongCapability(id));
        };
        let old = clickable.clickable_model().rollover_enabled();
        if !super::figure::widget::set_rollover_enabled(clickable.clickable_model_mut(), enabled) {
            return Ok(false);
        }
        self.record_property_change(
            id,
            "rollover_enabled",
            PropertyValue::Bool(old),
            PropertyValue::Bool(enabled),
        );
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn sync_clickable_visual(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        visual: ClickableVisualState,
    ) -> bool {
        let Some(clickable) = self
            .blocks
            .get_mut(id)
            .and_then(|block| block.figure.clickable_mut())
        else {
            return false;
        };
        if !super::figure::widget::sync_visual(clickable.clickable_model_mut(), visual) {
            return false;
        }
        self.repaint(update_manager, id, None);
        true
    }

    pub fn set_insets(&mut self, id: FigureId, insets: (f64, f64, f64, f64)) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.insets == insets {
            return false;
        }
        block.insets = insets;
        self.mark_validation_path_invalid(id);
        self.notify_block_changed(id);
        true
    }

    pub(crate) fn border_snapshot(&self, id: FigureId) -> Option<&BorderSnapshot> {
        self.blocks.get(id)?.border_snapshot.as_ref()
    }

    pub(crate) fn set_border_snapshot(&mut self, id: FigureId, snapshot: BorderSnapshot) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.border_snapshot.as_ref() == Some(&snapshot) {
            return false;
        }
        block.insets = snapshot.insets();
        block.border_snapshot = Some(snapshot);
        true
    }

    pub fn set_opaque(&mut self, id: FigureId, opaque: bool) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.is_opaque == opaque {
            return false;
        }
        block.is_opaque = opaque;
        self.notify_block_changed(id);
        true
    }

    pub fn set_figure_style(&mut self, id: FigureId, mut style: FigureStyle) -> bool {
        let Some(old_style) = self.blocks.get(id).map(|block| block.style.clone()) else {
            return false;
        };
        style.alpha = style.alpha.map(|alpha| alpha.clamp(0.0, 1.0));
        if old_style == style {
            return false;
        }
        self.blocks[id].style = style.clone();
        self.notify_block_changed(id);
        self.emit_style_property_changes(id, &old_style, &style);
        true
    }

    pub fn set_figure_style_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        style: FigureStyle,
    ) -> bool {
        let font_changed = self
            .blocks
            .get(id)
            .is_some_and(|block| block.style.font != style.font);
        if !self.set_figure_style(id, style) {
            return false;
        }

        let mut stack = vec![id];
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.blocks.get(node_id) else {
                continue;
            };
            stack.extend(node.children.iter().copied());
            if font_changed {
                self.mark_invalid(update_manager, node_id);
            }
            self.repaint(update_manager, node_id, None);
        }
        true
    }

    fn emit_style_property_changes(&mut self, id: FigureId, old: &FigureStyle, new: &FigureStyle) {
        if old.foreground != new.foreground {
            self.record_property_change(
                id,
                "foreground",
                old.foreground
                    .map_or(PropertyValue::None, PropertyValue::Color),
                new.foreground
                    .map_or(PropertyValue::None, PropertyValue::Color),
            );
        }
        if old.background != new.background {
            self.record_property_change(
                id,
                "background",
                old.background
                    .map_or(PropertyValue::None, PropertyValue::Color),
                new.background
                    .map_or(PropertyValue::None, PropertyValue::Color),
            );
        }
        if old.alpha != new.alpha {
            self.record_property_change(
                id,
                "alpha",
                old.alpha.map_or(PropertyValue::None, PropertyValue::Number),
                new.alpha.map_or(PropertyValue::None, PropertyValue::Number),
            );
        }
        if old.font != new.font {
            self.record_property_change(
                id,
                "font",
                old.font
                    .clone()
                    .map_or(PropertyValue::None, PropertyValue::Text),
                new.font
                    .clone()
                    .map_or(PropertyValue::None, PropertyValue::Text),
            );
        }
        if old.cursor != new.cursor {
            self.record_property_change(
                id,
                "cursor",
                old.cursor
                    .map_or(PropertyValue::None, PropertyValue::Cursor),
                new.cursor
                    .map_or(PropertyValue::None, PropertyValue::Cursor),
            );
        }
        if old.tooltip != new.tooltip {
            self.record_property_change(
                id,
                "tooltip",
                old.tooltip
                    .clone()
                    .flatten()
                    .map_or(PropertyValue::None, PropertyValue::Text),
                new.tooltip
                    .clone()
                    .flatten()
                    .map_or(PropertyValue::None, PropertyValue::Text),
            );
        }
    }

    /// 返回节点沿父链传播后的有效可见性。
    pub fn is_effectively_visible(&self, id: FigureId) -> bool {
        self.effective_flag_from(id, |block| block.is_visible)
    }

    /// 返回节点沿父链传播后的有效启用状态。
    pub fn is_effectively_enabled(&self, id: FigureId) -> bool {
        self.effective_flag_from(id, |block| block.is_enabled)
    }

    pub fn is_focusable(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| block.is_focusable)
    }

    pub fn is_focus_traversable(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|block| block.is_focus_traversable)
    }

    pub fn can_request_focus(&self, id: FigureId) -> bool {
        self.is_attached(id)
            && self.is_effectively_visible(id)
            && self.is_effectively_enabled(id)
            && self.is_focusable(id)
    }

    pub fn can_traverse_focus(&self, id: FigureId) -> bool {
        self.is_attached(id)
            && self.is_effectively_visible(id)
            && self.is_effectively_enabled(id)
            && self.is_focus_traversable(id)
    }

    pub(crate) fn can_retain_focus(&self, id: FigureId) -> bool {
        self.is_attached(id)
            && self.is_effectively_visible(id)
            && self.is_effectively_enabled(id)
            && (self.is_focusable(id) || self.is_focus_traversable(id))
    }

    pub fn set_focusable(&mut self, id: FigureId, focusable: bool) -> bool {
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

    pub fn set_focus_traversable(&mut self, id: FigureId, traversable: bool) -> bool {
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
    pub fn set_visible(&mut self, id: FigureId, visible: bool) -> bool {
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
            block_id: id,
            property: "visible",
            old_value: PropertyValue::Bool(old_value),
            new_value: PropertyValue::Bool(visible),
        });
        true
    }

    pub fn set_visible_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        visible: bool,
    ) -> bool {
        let Some((old_bounds, old_visual_bounds, parent_id, was_effectively_visible)) =
            self.blocks.get(id).map(|block| {
                (
                    block.figure_bounds(),
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
            self.erase(update_manager, id, old_bounds, old_visual_bounds, parent_id);
        }
        self.mark_invalid(update_manager, parent_id.unwrap_or(id));
        if visible {
            self.repaint(update_manager, id, None);
        }
        true
    }

    /// 设置块启用状态。
    pub fn set_enabled(&mut self, id: FigureId, enabled: bool) -> bool {
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
            block_id: id,
            property: "enabled",
            old_value: PropertyValue::Bool(old_value),
            new_value: PropertyValue::Bool(enabled),
        });
        true
    }

    pub fn set_enabled_with_update(
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

    /// 设置布局管理器
    pub fn set_layout_manager(&mut self, layout_manager: Box<dyn LayoutManager>) {
        let container_id = self.contents.unwrap_or(self.root);
        self.set_block_layout_manager(container_id, layout_manager);
    }

    /// 获取布局管理器
    pub fn get_layout_manager(&self) -> Option<&dyn LayoutManager> {
        self.blocks
            .get(self.contents.unwrap_or(self.root))
            .and_then(|block| block.layout.manager.as_deref())
    }

    /// 设置指定块的布局管理器
    pub fn set_block_layout_manager(
        &mut self,
        block_id: FigureId,
        layout_manager: Box<dyn LayoutManager>,
    ) {
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.layout.manager = Some(layout_manager);
        }
        self.mark_validation_path_invalid_for(block_id, LayoutInvalidation::Structure);
    }

    pub(crate) fn replace_block_layout_manager(
        &mut self,
        block_id: FigureId,
        layout_manager: Option<Box<dyn LayoutManager>>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.layout.manager.is_none() && layout_manager.is_none() {
            return false;
        }
        block.layout.manager = layout_manager;
        self.mark_validation_path_invalid_for(block_id, LayoutInvalidation::Structure);
        true
    }

    pub(crate) fn validate_layout_manager_constraints(
        &self,
        block_id: FigureId,
        layout_manager: &dyn LayoutManager,
    ) -> Result<(), LayoutError> {
        let Some(block) = self.blocks.get(block_id) else {
            return Ok(());
        };
        for (child, constraint) in &block.layout.constraints {
            layout_manager.validate_constraint(block_id, *child, constraint.as_ref())?;
        }
        Ok(())
    }

    /// 获取指定块的布局管理器
    pub fn get_block_layout_manager(&self, block_id: FigureId) -> Option<&dyn LayoutManager> {
        self.blocks
            .get(block_id)
            .and_then(|b| b.layout.manager.as_deref())
    }

    /// 设置父容器施加给直接子节点的布局约束。
    pub fn set_constraint<C>(&mut self, child_id: FigureId, constraint: C) -> bool
    where
        C: LayoutConstraint,
    {
        self.set_boxed_constraint(child_id, Box::new(constraint))
    }

    pub(crate) fn set_boxed_constraint(
        &mut self,
        child_id: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };
        parent.layout.constraints.insert(child_id, constraint);
        self.mark_validation_path_invalid_for(parent_id, LayoutInvalidation::Constraint);
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ConstraintChanged,
            container_id: parent_id,
            child_id: Some(child_id),
        });
        true
    }

    /// 获取指定类型的布局约束。
    pub fn get_constraint<C>(&self, child_id: FigureId) -> Option<&C>
    where
        C: LayoutConstraint,
    {
        self.constraint(child_id)?.as_any().downcast_ref::<C>()
    }

    /// 移除父容器为直接子节点保存的布局约束。
    pub fn remove_constraint(&mut self, child_id: FigureId) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let removed = self
            .blocks
            .get_mut(parent_id)
            .and_then(|parent| parent.layout.constraints.remove(&child_id))
            .is_some();
        if removed {
            self.mark_validation_path_invalid_for(parent_id, LayoutInvalidation::Constraint);
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::ConstraintChanged,
                container_id: parent_id,
                child_id: Some(child_id),
            });
        }
        removed
    }

    fn constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint> {
        let parent_id = self.blocks.get(child_id)?.parent?;
        self.blocks
            .get(parent_id)?
            .layout
            .constraints
            .get(&child_id)
            .map(Box::as_ref)
    }

    /// 使布局生效
    ///
    /// 对应 draw2d: validate()
    /// 标记布局为有效
    pub fn validate(&mut self) {
        let target = self.contents.unwrap_or(self.root);
        if let Some(block) = self.blocks.get_mut(target) {
            block.is_valid = true;
        }
    }

    // ========== 坐标变换方法 ==========

    /// 原始平移（对应 draw2d: primTranslate）
    ///
    /// Moves one node in its parent content domain.
    ///
    /// Descendant bounds remain unchanged. Their projected positions change
    /// through the shared parent transform.
    pub fn prim_translate(&mut self, block_id: FigureId, dx: f64, dy: f64) {
        let Some((old_bounds, new_bounds, has_children)) =
            self.blocks.get_mut(block_id).map(|block| {
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

        self.notify_block_changed(block_id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            block_id,
            old_bounds,
            new_bounds,
        });
        if has_children {
            self.emit_figure_event(FigureEvent::CoordinateSystemChanged {
                block_id,
                old_bounds,
                new_bounds,
            });
        }
        self.emit_ancestor_moved(block_id);
        self.mark_freeform_ancestor_extents_dirty(block_id);
    }

    fn emit_ancestor_moved(&mut self, ancestor_id: FigureId) {
        let mut stack = self
            .blocks
            .get(ancestor_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        while let Some(block_id) = stack.pop() {
            if let Some(block) = self.blocks.get(block_id) {
                stack.extend(block.children.iter().copied());
            }
            self.emit_ancestor_event(AncestorEvent {
                kind: AncestorEventKind::Moved,
                block_id,
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
    pub fn set_bounds(&mut self, block_id: FigureId, x: f64, y: f64, width: f64, height: f64) {
        let (old_bounds, has_children) = {
            if let Some(block) = self.blocks.get(block_id) {
                (block.figure_bounds(), !block.children.is_empty())
            } else {
                return;
            }
        };
        let resize = width != old_bounds.width || height != old_bounds.height;
        let translate = x != old_bounds.x || y != old_bounds.y;
        if !resize && !translate {
            return;
        }

        if let Some(block) = self.blocks.get_mut(block_id) {
            block.set_node_bounds(Rectangle::new(x, y, width, height));
        }
        self.notify_block_changed(block_id);
        let new_bounds = Rectangle::new(x, y, width, height);
        self.emit_figure_event(FigureEvent::FigureMoved {
            block_id,
            old_bounds,
            new_bounds,
        });
        if translate && has_children {
            self.emit_figure_event(FigureEvent::CoordinateSystemChanged {
                block_id,
                old_bounds,
                new_bounds,
            });
        }
        if translate {
            self.emit_ancestor_moved(block_id);
        }
        self.mark_freeform_ancestor_extents_dirty(block_id);
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
    pub fn set_bounds_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        block_id: FigureId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> bool {
        let Some(block) = self.blocks.get(block_id) else {
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
        let freeform_ancestor = self.nearest_freeform_ancestor(block_id);
        let visible = self.is_effectively_visible(block_id);

        if visible {
            self.erase(
                update_manager,
                block_id,
                old_bounds,
                old_visual_bounds,
                parent_id,
            );
        }

        self.set_bounds(block_id, x, y, width, height);

        if resize {
            self.mark_invalid(update_manager, block_id);
        } else if let Some(freeform_ancestor) = freeform_ancestor {
            update_manager.add_invalid_figure(freeform_ancestor);
        }

        if visible {
            self.repaint(update_manager, block_id, None);
        }

        true
    }

    fn erase(
        &self,
        update_manager: &mut UpdateManager,
        block_id: FigureId,
        old_bounds: Rectangle,
        mut old_visual_bounds: Rectangle,
        parent_id: Option<FigureId>,
    ) {
        let Some(parent_id) = parent_id else {
            return;
        };
        if !self.blocks.contains_key(block_id) || !self.blocks.contains_key(parent_id) {
            return;
        }

        old_visual_bounds.translate(old_bounds.x, old_bounds.y);
        if let Some(parent) = self.blocks.get(parent_id) {
            parent.child_transform().apply_to(&mut old_visual_bounds);
        }
        update_manager.add_dirty_region(parent_id, old_visual_bounds);
    }

    /// 将 node-local 几何转换到 logical surface domain。
    pub fn translate_to_absolute_mut<T: Translatable>(&self, block_id: FigureId, t: &mut T) {
        if let Some(transform) = self.local_to_surface_transform(block_id) {
            t.transform(transform);
        }
    }

    /// 将 node-local 几何转换到 parent content domain。
    pub fn translate_to_parent<T: Translatable>(&self, block_id: FigureId, t: &mut T) {
        if let Some(block) = self.blocks.get(block_id) {
            let bounds = block.figure_bounds();
            t.transform(novadraw_geometry::Affine2D::from_translation(
                bounds.x, bounds.y,
            ));
        }
    }

    /// 将 parent content 几何转换到 node-local domain。
    pub fn translate_from_parent<T: Translatable>(&self, block_id: FigureId, t: &mut T) {
        if let Some(block) = self.blocks.get(block_id) {
            let bounds = block.figure_bounds();
            t.transform(novadraw_geometry::Affine2D::from_translation(
                -bounds.x, -bounds.y,
            ));
        }
    }

    /// 将 logical surface 几何转换到 node-local domain。
    pub fn translate_to_relative<T: Translatable>(&self, block_id: FigureId, t: &mut T) -> bool {
        let Some(transform) = self
            .local_to_surface_transform(block_id)
            .and_then(|transform| transform.inverse())
        else {
            return false;
        };
        t.transform(transform);
        true
    }

    /// 返回 node-local 到 logical surface 的完整父链变换。
    pub fn local_to_surface_transform(
        &self,
        block_id: FigureId,
    ) -> Option<novadraw_geometry::Affine2D> {
        let mut transform = novadraw_geometry::Affine2D::IDENTITY;
        let mut current_id = block_id;

        loop {
            let current = self.blocks.get(current_id)?;
            let bounds = current.figure_bounds();
            transform =
                novadraw_geometry::Affine2D::from_translation(bounds.x, bounds.y) * transform;

            let Some(parent_id) = current.parent else {
                break;
            };
            let parent = self.blocks.get(parent_id)?;
            transform = parent.child_transform().affine() * transform;
            current_id = parent_id;
        }

        Some(transform)
    }

    /// Returns the complete child-content to logical-surface transform.
    pub fn child_content_to_surface_transform(
        &self,
        block_id: FigureId,
    ) -> Option<novadraw_geometry::Affine2D> {
        let block = self.blocks.get(block_id)?;
        Some(self.local_to_surface_transform(block_id)? * block.child_transform().affine())
    }
}

impl FigureTree {
    fn nearest_freeform_ancestor(&self, block_id: FigureId) -> Option<FigureId> {
        let parent = self.blocks.get(block_id)?.parent?;
        self.blocks[parent].layout.freeform.as_ref()?;
        Some(parent)
    }

    fn mark_freeform_ancestor_extents_dirty(&mut self, block_id: FigureId) {
        let mut current = self.blocks.get(block_id).and_then(|block| block.parent);
        while let Some(id) = current {
            let Some(block) = self.blocks.get_mut(id) else {
                break;
            };
            if block.layout.freeform.is_none() {
                break;
            }
            block.is_valid = false;
            block.layout.invalidate(LayoutInvalidation::Geometry);
            current = block.parent;
        }
    }

    fn mark_validation_path_invalid(&mut self, block_id: FigureId) {
        self.mark_validation_path_invalid_for(block_id, LayoutInvalidation::Geometry);
    }

    fn mark_validation_path_invalid_for(
        &mut self,
        mut block_id: FigureId,
        reason: LayoutInvalidation,
    ) {
        let mut invalidated = Vec::new();
        loop {
            let (parent, was_valid) = if let Some(block) = self.blocks.get_mut(block_id) {
                let was_valid = block.is_valid;
                block.is_valid = false;
                block.layout.invalidate(reason);
                if was_valid && let Some(lifecycle) = block.figure.lifecycle() {
                    lifecycle.invalidate();
                }
                (block.parent, was_valid)
            } else {
                (None, false)
            };

            if was_valid {
                invalidated.push(block_id);
            }
            match parent {
                Some(parent_id) => block_id = parent_id,
                None => break,
            }
        }
        for container_id in invalidated {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Invalidated,
                container_id,
                child_id: None,
            });
        }
    }

    pub(crate) fn invalid_block_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| (!block.is_valid).then_some(id))
            .collect()
    }

    fn effective_flag_from(
        &self,
        mut block_id: FigureId,
        local_flag: fn(&FigureNode) -> bool,
    ) -> bool {
        for _ in 0..self.blocks.len() {
            let Some(block) = self.blocks.get(block_id) else {
                return false;
            };
            if !local_flag(block) {
                return false;
            }
            let Some(parent_id) = block.parent else {
                return true;
            };
            block_id = parent_id;
        }
        false
    }
}

impl super::layout::LayoutContext for FigureTree {
    fn get_children(&self, parent_id: FigureId) -> Vec<(FigureId, Rectangle)> {
        if let Some(block) = self.blocks.get(parent_id) {
            block
                .children
                .iter()
                .filter_map(|&child_id| {
                    self.blocks
                        .get(child_id)
                        .map(|child| (child_id, child.figure_bounds()))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn get_constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint> {
        self.constraint(child_id)
    }

    fn get_preferred_size(&self, block_id: FigureId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.preferred_size(block_id, w_hint, h_hint)
            .unwrap_or((0.0, 0.0))
    }

    fn get_minimum_size(&self, block_id: FigureId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.minimum_size(block_id, w_hint, h_hint)
            .unwrap_or((0.0, 0.0))
    }

    fn get_maximum_size(&self, block_id: FigureId) -> (f64, f64) {
        self.maximum_size(block_id)
            .unwrap_or((f64::INFINITY, f64::INFINITY))
    }

    fn get_container_bounds(&self, container_id: FigureId) -> Rectangle {
        if let Some(block) = self.blocks.get(container_id) {
            block.client_area()
        } else {
            Rectangle::new(0.0, 0.0, 0.0, 0.0)
        }
    }

    fn get_freeform_extent(&self, block_id: FigureId) -> Option<Rectangle> {
        self.freeform_extent(block_id).ok()
    }

    fn get_content_scale(&self, block_id: FigureId) -> Option<f64> {
        self.blocks
            .get(block_id)
            .and_then(|block| block.figure.content_scale())
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
    use crate::style::{CursorIcon, FigureStyle, ResolvedStyle};
    use crate::{
        EllipseFigure, Figure, FigureEvent, FigureEventHandler, FigureId, FigureLifecycle,
        FigureTree, LineBorder, NotificationEffect, PolygonFigure, PolylineFigure, Rectangle,
        RootFigure, RoundedRectangleFigure, ScalableLayeredPaneFigure, TriangleFigure,
        UpdateManager, ViewportFigure,
    };
    use novadraw_core::Color as NovadrawCoreColor;
    use novadraw_geometry::Vec2;
    use novadraw_render::{NdCanvas, command::RenderCommandKind};

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

    fn rect_signature(rect: &[glam::DVec2; 2]) -> [f64; 4] {
        [rect[0].x, rect[0].y, rect[1].x, rect[1].y]
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

        fn line_cap(&self) -> novadraw_render::command::LineCap {
            novadraw_render::command::LineCap::default()
        }

        fn line_join(&self) -> novadraw_render::command::LineJoin {
            novadraw_render::command::LineJoin::default()
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
            Some(NovadrawCoreColor::hex("#44aa44"))
        }

        fn line_cap(&self) -> novadraw_render::command::LineCap {
            novadraw_render::command::LineCap::default()
        }

        fn line_join(&self) -> novadraw_render::command::LineJoin {
            novadraw_render::command::LineJoin::default()
        }

        fn fill_shape(&self, gc: &mut NdCanvas) {
            gc.fill_rect(
                self.paint_rect.x,
                self.paint_rect.y,
                self.paint_rect.width,
                self.paint_rect.height,
                NovadrawCoreColor::hex("#44aa44"),
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
        Attached(FigureId),
        Detached(FigureId),
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
        fn on_attached(&mut self, parent_id: FigureId) {
            self.events
                .lock()
                .unwrap()
                .push(LifecycleEvent::Attached(parent_id));
        }

        fn on_detached(&mut self, parent_id: FigureId) {
            self.events
                .lock()
                .unwrap()
                .push(LifecycleEvent::Detached(parent_id));
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

        fn line_cap(&self) -> novadraw_render::command::LineCap {
            novadraw_render::command::LineCap::default()
        }

        fn line_join(&self) -> novadraw_render::command::LineJoin {
            novadraw_render::command::LineJoin::default()
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

        fn line_cap(&self) -> novadraw_render::command::LineCap {
            novadraw_render::command::LineCap::default()
        }

        fn line_join(&self) -> novadraw_render::command::LineJoin {
            novadraw_render::command::LineJoin::default()
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
            gc.fill_rect(
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
                novadraw_render::command::RenderCommandKind::FillRect { .. }
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
    fn test_figure_lifecycle_hooks_fire_on_add_remove_and_reparent() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut scene = FigureTree::new();
        let left_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let right_id = scene.add_child_to(
            left_id,
            Box::new(RectangleFigure::new(20.0, 20.0, 50.0, 50.0)),
        );
        let child_id = scene.add_child_to(
            left_id,
            Box::new(LifecycleRecordingFigure::new(Arc::clone(&events))),
        );

        assert_eq!(
            *events.lock().unwrap(),
            vec![LifecycleEvent::Attached(left_id)]
        );

        assert!(scene.detach_child(left_id, child_id));
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                LifecycleEvent::Attached(left_id),
                LifecycleEvent::Detached(left_id)
            ]
        );

        assert!(scene.attach_child_checked(right_id, child_id).is_ok());
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                LifecycleEvent::Attached(left_id),
                LifecycleEvent::Detached(left_id),
                LifecycleEvent::Attached(right_id)
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
            block_id: parent_id
        }));
        assert!(!effects.contains(&NotificationEffect::Notify { block_id: child_id }));
        assert!(
            effects.contains(&NotificationEffect::EmitFigure(FigureEvent::FigureMoved {
                block_id: parent_id,
                old_bounds: Rectangle::new(0.0, 0.0, 100.0, 100.0),
                new_bounds: Rectangle::new(10.0, 20.0, 100.0, 100.0),
            }))
        );
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                NotificationEffect::EmitFigure(FigureEvent::FigureMoved { block_id, .. })
                    if *block_id == child_id
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
                block_id: root_id,
                old_bounds: Rectangle::new(0.0, 0.0, 100.0, 100.0),
                new_bounds: Rectangle::new(5.0, 10.0, 100.0, 100.0),
            }
        )));
        assert!(!effects.iter().any(|effect| {
            matches!(
                effect,
                NotificationEffect::EmitFigure(FigureEvent::FigureMoved { block_id, .. })
                    if *block_id == child_id
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

    // ========== translate_to_parent 测试 ==========

    /// 测试 translate_to_parent 基本功能
    ///
    /// 场景：当前节点是坐标根且无 insets
    /// 期望：本地坐标 (10, 20) 转换为父坐标 (30, 50)
    #[test]
    fn test_translate_to_parent_basic() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );

        let mut point = (10.0, 20.0);
        scene.translate_to_parent(coord_root_id, &mut point);
        assert_eq!(point, (30.0, 50.0));
    }

    /// Node placement 不包含其 child-content insets。
    #[test]
    fn test_translate_to_parent_with_insets() {
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
        scene.translate_to_parent(coord_root_id, &mut point);
        assert_eq!(point.0, 30.0);
        assert_eq!(point.1, 50.0);
    }

    /// 测试 translate_to_parent 父节点不是坐标根
    ///
    /// 场景：当前节点不是坐标根
    /// 期望：不进行转换，返回原坐标
    #[test]
    fn test_translate_to_parent_not_coordinate_root() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.add_child_to(contents_id, Box::new(parent));

        let child = RectangleFigure::new(10.0, 20.0, 50.0, 50.0);
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        let mut point = (10.0, 20.0);
        scene.translate_to_parent(child_id, &mut point);
        assert_eq!(point, (20.0, 40.0));
    }

    // ========== translate_from_parent 测试 ==========

    /// 测试 translate_from_parent 基本功能
    ///
    /// 场景：当前节点是坐标根且无 insets
    /// 期望：父坐标 (30, 50) 转换为本地坐标 (10, 20)
    #[test]
    fn test_translate_from_parent_basic() {
        let mut scene = FigureTree::new();

        let contents = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
        let contents_id = scene.set_contents(Box::new(contents));

        let coord_root_id = scene.add_child_to(
            contents_id,
            Box::new(TestCoordinateRootFigure::new(20.0, 30.0, 100.0, 100.0)),
        );
        let mut point = (30.0, 50.0);
        scene.translate_from_parent(coord_root_id, &mut point);
        assert_eq!(point, (10.0, 20.0));
    }

    /// Parent content 到 node local 只逆转 node placement。
    #[test]
    fn test_translate_from_parent_with_insets() {
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
        scene.translate_from_parent(coord_root_id, &mut point);
        assert_eq!(point.0, 15.0);
        assert_eq!(point.1, 25.0);
    }

    // ========== translate_to_relative 测试 ==========

    /// 测试 translate_to_relative 基本功能
    ///
    /// 场景：父节点是坐标根，bounds = (0, 0)
    /// 期望：绝对坐标 (30, 40) 转换为本地坐标 (30, 40)
    #[test]
    fn test_translate_to_relative_basic() {
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
        scene.translate_to_relative(child_id, &mut point);
        assert_eq!(point, (0.0, 0.0));
    }

    /// 测试 translate_to_relative 嵌套坐标根
    ///
    /// 场景：深层嵌套，多个坐标根
    /// 期望：正确累积转换
    #[test]
    fn test_translate_to_relative_nested() {
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
        scene.translate_to_relative(child_id, &mut point);
        assert_eq!(point, (0.0, 0.0));
    }

    /// 测试 translate_to_relative 与 translate_to_absolute_mut 严格互为父链逆变换。
    ///
    /// 场景：目标节点本身也是坐标根。
    /// 期望：转换到 absolute 后再转换回 relative 时，不会额外应用目标节点自己的
    /// translateFromParent；这与 Draw2D Figure#translateToRelative 的 parent-chain 协议一致。
    #[test]
    fn test_translate_to_relative_roundtrips_target_coordinate_root() {
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
        scene.translate_to_absolute_mut(coord_root2_id, &mut point);
        assert_eq!(point, (45.0, 60.0));

        scene.translate_to_relative(coord_root2_id, &mut point);
        assert_eq!(point, (15.0, 25.0));
    }

    /// 测试 translate_to_relative Rectangle 类型
    ///
    /// 场景：使用 Rectangle 类型进行坐标转换
    /// 期望：Rectangle 的 x, y 被正确转换
    #[test]
    fn test_translate_to_relative_rect() {
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
        scene.translate_to_relative(child_id, &mut rect);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
    }

    // ========== translate_to_absolute_mut 测试 ==========

    /// 测试 translate_to_absolute_mut 基本功能
    ///
    /// 场景：父节点是坐标根，bounds = (20, 30)
    /// 期望：本地坐标 (10, 5) 转换为绝对坐标 (30, 35)
    #[test]
    fn test_translate_to_absolute_mut_basic() {
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
        scene.translate_to_absolute_mut(child_id, &mut point);
        assert_eq!(point, (30.0, 35.0));
    }

    /// 测试 translate_to_absolute_mut 在坐标根包含 insets 时会通过父链协议叠加它们。
    #[test]
    fn test_translate_to_absolute_mut_includes_parent_insets() {
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
        scene.translate_to_absolute_mut(child_id, &mut point);
        assert_eq!(point, (37.0, 40.0));
    }

    /// 测试 translate_to_absolute_mut 嵌套坐标根
    ///
    /// 场景：多层坐标根
    /// 期望：正确累加多个坐标根的 bounds
    #[test]
    fn test_translate_to_absolute_mut_nested() {
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
        scene.translate_to_absolute_mut(child_id, &mut point);
        assert_eq!(point, (30.0, 55.0));
    }

    /// 测试 translate_to_absolute_mut 在多层坐标根且包含 insets 时严格按父链协议累加。
    #[test]
    fn test_translate_to_absolute_mut_nested_insets_follow_parent_chain_protocol() {
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
        scene.translate_to_absolute_mut(child_id, &mut point);
        assert_eq!(point, (39.0, 61.0));
    }

    /// 测试 translate_to_absolute_mut Rectangle 类型
    ///
    /// 场景：使用 Rectangle 类型进行坐标转换
    /// 期望：Rectangle 的 x, y 被正确转换
    #[test]
    fn test_translate_to_absolute_mut_rect() {
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
        scene.translate_to_absolute_mut(child_id, &mut rect);
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
                LineBorder::new(NovadrawCoreColor::hex("#111111"), 2.0)
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
                NovadrawCoreColor::hex("#222222"),
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
                LineBorder::new(NovadrawCoreColor::hex("#111111"), 2.0)
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
                NovadrawCoreColor::hex("#222222"),
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
                RenderCommandKind::FillRect { rect, color }
                    if rect_signature(rect) == [20.0, 0.0, 30.0, 10.0] =>
                {
                    Some(*color)
                }
                _ => None,
            })
            .expect("second sibling must paint");

        assert_eq!(sibling_color.a, 1.0);
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
                            Vec2::new(0.0, 0.0),
                            Vec2::new(100.0, 0.0),
                            Vec2::new(100.0, 100.0),
                            Vec2::new(0.0, 100.0),
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
                LineBorder::new(NovadrawCoreColor::hex("#111111"), 2.0)
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
        assert_eq!(scene.get_contents(), Some(replacement));
        assert_eq!(scene.child_order(scene.root), Some(vec![replacement]));
    }

    #[test]
    fn node_style_inherits_each_unset_property_independently() {
        let mut scene = FigureTree::new();
        let parent = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            scene.add_child_to(parent, Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)));
        let foreground = NovadrawCoreColor::hex("#123456");
        let background = NovadrawCoreColor::hex("#abcdef");

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
                RenderCommandKind::FillRect { rect, color }
                    if rect_signature(rect) == [20.0, 0.0, 30.0, 10.0] =>
                {
                    Some(*color)
                }
                _ => None,
            })
            .expect("styled descendant must paint");

        assert_eq!(color.a, 0.5);
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
                    if event.block_id == parent && event.property == property
            )));
        }
        assert_eq!(
            scene.resolved_style(child).unwrap().cursor,
            CursorIcon::Crosshair
        );
    }
}
