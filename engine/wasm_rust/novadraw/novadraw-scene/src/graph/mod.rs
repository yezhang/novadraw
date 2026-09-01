//! 场景图管理
//!
//! 提供场景图数据结构和管理功能。

use std::{
    collections::HashSet,
    error::Error,
    fmt,
    ops::{Deref, DerefMut},
};

use novadraw_core::Color;
use novadraw_geometry::{Rectangle, Translatable};
use novadraw_render::{
    NdCanvas,
    command::{LineCap, LineJoin},
};
use slotmap::{Key, SlotMap};
use uuid::Uuid;

use super::figure::{ChildPolicy, Updatable};
use super::layout::{LayoutConstraint, LayoutManager};
use crate::runtime::update::{
    AncestorEvent, AncestorEventKind, FigureEvent, LayoutEvent, LayoutEventKind,
    NotificationEffect, NotificationQueue, PropertyChangeEvent, PropertyValue, UpdateManager,
};
use crate::{PendingMutationBatch, mutation::PendingMutationKind};

// 渲染模块
pub mod render_recursive;

pub use render_recursive::{FigureGraphRenderRef, FigureRenderer};

#[cfg(test)]
pub mod bounds_test;

#[cfg(test)]
pub mod update_integration_test;

slotmap::new_key_type! { pub struct BlockId; }

/// Runtime-local, generational identity of a Figure node.
///
/// `BlockId` remains as a compatibility name while callers migrate to the
/// architecture-level `FigureId` terminology.
pub type FigureId = BlockId;

/// Figure 树允许的最大深度。根节点深度为 0。
pub const MAX_TREE_DEPTH: usize = 10_000;

/// Figure 树结构变更失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphMutationError {
    ParentNotFound,
    ChildNotFound,
    CycleDetected,
    DuplicateChild,
    ChildLimitExceeded { limit: usize },
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
            Self::InvalidParentRelation => write!(f, "child is not attached to expected parent"),
            Self::DepthLimitExceeded { limit } => {
                write!(f, "figure tree depth exceeds limit {limit}")
            }
        }
    }
}

impl Error for GraphMutationError {}

const SELECTION_OUTLINE_COLOR: Color = Color {
    r: 0.98,
    g: 0.86,
    b: 0.22,
    a: 1.0,
};
const SELECTION_OUTLINE_INSET: f64 = 2.0;
const SELECTION_OUTLINE_STROKE_WIDTH: f64 = 4.0;

fn point_in_rect(point: (f64, f64), rect: &Rectangle) -> bool {
    point.0 >= rect.x
        && point.0 <= rect.x + rect.width
        && point.1 >= rect.y
        && point.1 <= rect.y + rect.height
}

pub(crate) fn paint_selection_overlay(block: &FigureBlock, selected: bool, gc: &mut NdCanvas) {
    if !selected {
        return;
    }

    let bounds = block.figure_bounds();
    let width = (bounds.width - SELECTION_OUTLINE_INSET - SELECTION_OUTLINE_INSET).max(0.0);
    let height = (bounds.height - SELECTION_OUTLINE_INSET - SELECTION_OUTLINE_INSET).max(0.0);
    gc.stroke_rect(
        SELECTION_OUTLINE_INSET,
        SELECTION_OUTLINE_INSET,
        width,
        height,
        SELECTION_OUTLINE_COLOR,
        SELECTION_OUTLINE_STROKE_WIDTH,
        LineCap::default(),
        LineJoin::default(),
    );
}

/// State shared by every Figure node.
///
/// Concrete Figure implementations retain only type-specific data. Tree,
/// interaction and update algorithms consume this state without downcasting.
pub struct NodeState {
    pub(crate) bounds: Rectangle,
    pub(crate) insets: (f64, f64, f64, f64),
    pub(crate) is_visible: bool,
    pub(crate) is_enabled: bool,
    pub(crate) is_opaque: bool,
    pub(crate) is_valid: bool,
    pub(crate) preferred_size: Option<(f64, f64)>,
    pub(crate) minimum_size: Option<(f64, f64)>,
    pub(crate) maximum_size: Option<(f64, f64)>,
    pub(crate) style: StyleOverride,
}

/// Inheritable rendering properties explicitly set on one node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StyleOverride {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub alpha: Option<f64>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            bounds: Rectangle::ZERO,
            insets: (0.0, 0.0, 0.0, 0.0),
            is_visible: true,
            is_enabled: true,
            is_opaque: false,
            is_valid: false,
            preferred_size: None,
            minimum_size: None,
            maximum_size: None,
            style: StyleOverride::default(),
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

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }

    pub fn style(&self) -> StyleOverride {
        self.style
    }
}

/// Container-owned layout strategy and child constraints.
#[derive(Default)]
pub struct LayoutState {
    pub(crate) manager: Option<Box<dyn LayoutManager>>,
    pub(crate) constraints: std::collections::HashMap<FigureId, Box<dyn LayoutConstraint>>,
}

impl LayoutState {
    pub fn has_manager(&self) -> bool {
        self.manager.is_some()
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
}

/// FigureNode - 图形节点
///
/// 场景图中的基本单元，同时包含图形数据（通过 Box<dyn Figure>）
/// 和树形结构（parent/children），参考 Eclipse Draw2D 的 Figure 设计。
///
/// # 与 Figure trait 的区别
///
/// - `FigureBlock` 是具体的数据结构，实现了树形节点的所有功能
/// - `dyn Figure` 是渲染接口 trait，定义了图形的几何和渲染行为
/// - 一个 `FigureBlock` 持有 `Box<dyn Figure>` 来实现具体的图形类型
pub struct FigureNode {
    /// 块 ID
    pub(crate) id: BlockId,
    /// UUID
    pub(crate) uuid: Uuid,
    /// 子块列表
    pub(crate) children: Vec<BlockId>,
    /// 父块
    pub(crate) parent: Option<BlockId>,
    /// 从 FigureGraph 根节点开始计算的深度；根节点深度为 0。
    pub(crate) depth: usize,
    /// 图形
    pub(crate) figure: Box<dyn super::Figure>,
    /// 容器布局策略、关系约束和后续布局缓存的唯一归属。
    pub(crate) layout: LayoutState,
    /// 所有 Figure 共享的节点状态。
    pub(crate) state: NodeState,
}

/// Compatibility name for the pre-runtime architecture.
pub type FigureBlock = FigureNode;

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
    pub fn id(&self) -> BlockId {
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

    fn set_figure_bounds(&mut self, bounds: Rectangle) {
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
        super::ChildTransform::from_affine(
            novadraw_geometry::Affine2D::from_translation(left, top)
                * self.figure.child_transform().affine(),
        )
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

#[inline]
fn rect_intersects(a: &Rectangle, b: &Rectangle) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

/// 场景图
///
/// 管理所有图形块的层次结构，参考 Eclipse Draw2d 设计模式。
///
/// # 使用示例
///
/// ```
/// use novadraw_scene::{Figure, RectangleFigure, FigureGraph};
///
/// let mut scene = FigureGraph::new();
///
/// // 创建根内容块（类似 Draw2d 的 setContents）
/// let contents = RectangleFigure::new(0.0, 0.0, 100.0, 50.0);
/// let contents_id = scene.set_contents(Box::new(contents));
///
/// // 添加子块到指定父块（类似 Draw2d 的 parent.addChild(child)）
/// let child = RectangleFigure::new(10.0, 10.0, 80.0, 30.0);
/// scene.add_child_to(contents_id, Box::new(child));
/// ```
pub struct FigureGraph {
    blocks: SlotMap<BlockId, FigureBlock>,
    uuid_map: std::collections::HashMap<Uuid, BlockId>,
    /// 根块（内部使用）
    root: BlockId,
    /// 内容块（用户可访问的根容器）
    contents: Option<BlockId>,
    /// Compatibility selection model kept outside core node state.
    selected: HashSet<BlockId>,
    notification_effects: NotificationQueue,
}

/// Compatibility name while the public API migrates from graph to tree.
pub type FigureTree = FigureGraph;

impl FigureGraph {
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

        FigureGraph {
            blocks,
            uuid_map: std::collections::HashMap::new(),
            root: root_id,
            contents: None,
            selected: HashSet::new(),
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

    fn notify_block_changed(&mut self, block_id: BlockId) {
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
        block_id: BlockId,
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

    pub(crate) fn record_coordinate_system_changed(&mut self, block_id: BlockId) {
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
    pub fn set_contents(&mut self, figure: Box<dyn super::Figure>) -> BlockId {
        let contents_id = self
            .new_block_with_parent(figure, self.root)
            .expect("FigureGraph root must exist");
        self.contents = Some(contents_id);
        self.invalidate();
        contents_id
    }

    /// 获取内容块
    pub fn get_contents(&self) -> Option<BlockId> {
        self.contents
    }

    /// 一次性把旧版“相对最近坐标根的绝对 bounds”转换为 parent-local bounds。
    ///
    /// `legacy_coordinate_roots` 必须包含旧模型中所有会重置子树坐标域的节点。
    /// 该方法只用于场景导入迁移，应在 Runtime 接管场景和注册监听器前调用；
    /// 它不保留任何运行时兼容模式。返回实际改写的节点数量。
    pub fn migrate_legacy_bounds_to_parent_local(
        &mut self,
        legacy_coordinate_roots: &std::collections::HashSet<FigureId>,
    ) -> usize {
        let Some(contents_id) = self.contents else {
            return 0;
        };
        let Some(contents) = self.blocks.get(contents_id) else {
            return 0;
        };

        let mut changed = 0;
        let mut stack = vec![(contents_id, contents.figure_bounds())];
        while let Some((parent_id, original_parent_bounds)) = stack.pop() {
            let Some(parent) = self.blocks.get(parent_id) else {
                continue;
            };
            let children = parent.children.clone();
            let (top, left, _, _) = parent.state.insets;
            let resets_child_domain = legacy_coordinate_roots.contains(&parent_id);

            for child_id in children {
                let Some(original_child_bounds) =
                    self.blocks.get(child_id).map(FigureNode::figure_bounds)
                else {
                    continue;
                };
                stack.push((child_id, original_child_bounds));

                if resets_child_domain {
                    continue;
                }

                let migrated = Rectangle::new(
                    original_child_bounds.x - original_parent_bounds.x - left,
                    original_child_bounds.y - original_parent_bounds.y - top,
                    original_child_bounds.width,
                    original_child_bounds.height,
                );
                if migrated != original_child_bounds {
                    if let Some(child) = self.blocks.get_mut(child_id) {
                        child.set_figure_bounds(migrated);
                    }
                    changed += 1;
                }
            }
        }

        changed
    }

    /// 添加子块到指定父块
    ///
    /// 对应 draw2d: parent.addChild(child) (不触发 revalidate)
    ///
    /// 与 `add_child()` 的区别：此方法不触发 revalidate()，用于批量构建场景。
    pub fn add_child_to(&mut self, parent_id: BlockId, figure: Box<dyn super::Figure>) -> BlockId {
        self.try_add_child_to(parent_id, figure)
            .unwrap_or_else(|_| BlockId::null())
    }

    /// 尝试添加子块到指定父块。
    ///
    /// parent 不存在或深度超限时不分配节点、不修改 UUID 映射，并返回错误。
    pub fn try_add_child_to(
        &mut self,
        parent_id: BlockId,
        figure: Box<dyn super::Figure>,
    ) -> Result<BlockId, GraphMutationError> {
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
    /// use novadraw_scene::{figure::RectangleFigure, FigureGraph};
    ///
    /// let mut scene = FigureGraph::new();
    /// let parent_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
    /// let color = Color::hex("#3498db");
    /// // 添加子节点，bounds 位于 parent content domain
    /// let _child_id = scene.add_child_with_bounds(parent_id, 10.0, 10.0, 50.0, 50.0, color);
    /// ```
    pub fn add_child_with_bounds(
        &mut self,
        parent_id: BlockId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: novadraw_core::Color,
    ) -> BlockId {
        let figure = super::figure::RectangleFigure::new_with_color(x, y, width, height, color);
        self.try_add_child_to(parent_id, Box::new(figure))
            .unwrap_or_else(|_| BlockId::null())
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
    pub fn add_child(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        parent_id: BlockId,
        figure: Box<dyn super::Figure>,
    ) -> BlockId {
        let child_id = match self.try_add_child_to(parent_id, figure) {
            Ok(child_id) => child_id,
            Err(_) => return BlockId::null(),
        };
        let visual_bounds = self.blocks[child_id].visual_bounds();

        self.mark_invalid(update_manager, parent_id);
        update_manager.add_dirty_region(child_id, visual_bounds);
        self.mark_invalid(update_manager, child_id);

        child_id
    }

    pub fn apply_pending_mutations(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        mutations: PendingMutationBatch,
    ) -> bool {
        if mutations.is_empty() {
            return false;
        }

        let mut changed = false;
        for mutation in mutations.into_vec() {
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
            };
        }

        changed
    }

    /// Removes a direct child through the update transaction.
    pub fn remove_child(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        parent: BlockId,
        child: BlockId,
    ) -> bool {
        self.apply_remove_mutation(
            update_manager,
            PendingMutationKind::RemoveChild { parent, child },
        )
    }

    /// Reparents a block through the update transaction.
    pub fn reparent(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        child: BlockId,
        new_parent: BlockId,
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
        parent_id: BlockId,
    ) -> Result<BlockId, GraphMutationError> {
        let bounds = figure.bounds();
        let insets = figure.insets();
        let parent_depth = self
            .blocks
            .get(parent_id)
            .map(|parent| parent.depth)
            .ok_or(GraphMutationError::ParentNotFound)?;
        let parent = &self.blocks[parent_id];
        if parent.figure.child_policy() == ChildPolicy::Single && !parent.children.is_empty() {
            return Err(GraphMutationError::ChildLimitExceeded { limit: 1 });
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
            layout: LayoutState::default(),
            state: NodeState {
                bounds,
                insets,
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

    fn attach_child_checked(
        &mut self,
        parent_id: BlockId,
        child_id: BlockId,
    ) -> Result<(), GraphMutationError> {
        let new_depth = self.validate_attachment(parent_id, child_id)?;

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

    fn detach_child(&mut self, parent_id: BlockId, child_id: BlockId) -> bool {
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };

        let old_len = parent.children.len();
        parent.children.retain(|&id| id != child_id);
        if parent.children.len() == old_len {
            return false;
        }
        parent.layout.constraints.remove(&child_id);

        if let Some(child) = self.blocks.get_mut(child_id)
            && let Some(lifecycle) = child.figure.lifecycle()
        {
            lifecycle.on_detached(parent_id);
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
        parent_id: BlockId,
        child_id: BlockId,
    ) -> Result<usize, GraphMutationError> {
        let parent = self
            .blocks
            .get(parent_id)
            .ok_or(GraphMutationError::ParentNotFound)?;
        if !self.blocks.contains_key(child_id) {
            return Err(GraphMutationError::ChildNotFound);
        }

        if parent_id == child_id || self.is_descendant_of(parent_id, child_id) {
            return Err(GraphMutationError::CycleDetected);
        }
        if parent.children.contains(&child_id) {
            return Err(GraphMutationError::DuplicateChild);
        }
        if parent.figure.child_policy() == ChildPolicy::Single && !parent.children.is_empty() {
            return Err(GraphMutationError::ChildLimitExceeded { limit: 1 });
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

    fn subtree_height(&self, root_id: BlockId) -> usize {
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

    fn set_subtree_depth(&mut self, root_id: BlockId, root_depth: usize) {
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

    fn contains_direct_child(&self, parent_id: BlockId, child_id: BlockId) -> bool {
        self.blocks
            .get(parent_id)
            .is_some_and(|parent| parent.children.contains(&child_id))
    }

    fn is_descendant_of(&self, mut node: BlockId, ancestor: BlockId) -> bool {
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
        update_manager: &mut dyn UpdateManager,
        mutation: PendingMutationKind,
    ) -> bool {
        let PendingMutationKind::RemoveChild { parent, child } = mutation else {
            return false;
        };
        if !self.blocks.contains_key(child) {
            return false;
        }

        if !self.detach_child(parent, child) {
            return false;
        }

        if self.contents == Some(child) {
            self.contents = None;
        }

        self.clear_selection_for_subtree(child);
        self.mark_invalid(update_manager, parent);
        self.repaint(update_manager, parent, None);
        true
    }

    fn apply_reparent_mutation(
        &mut self,
        update_manager: &mut dyn UpdateManager,
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

    fn apply_add_mutation(
        &mut self,
        update_manager: &mut dyn UpdateManager,
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
    pub fn mark_invalid(&mut self, update_manager: &mut dyn UpdateManager, block_id: BlockId) {
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
        update_manager: &mut dyn UpdateManager,
        block_id: BlockId,
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
    pub fn repaint_all(&mut self, update_manager: &mut dyn UpdateManager) {
        if let Some(contents_id) = self.contents {
            self.repaint(update_manager, contents_id, None);
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
    pub fn perform_update(&mut self, update_manager: &mut dyn UpdateManager) -> NdCanvas {
        let mut canvas = NdCanvas::new();
        update_manager.perform_update(self, &mut canvas);
        canvas
    }

    /// 执行 validation phase 的图级语义。
    ///
    /// UpdateManager 只提供待验证队列与 phase 触发，
    /// FigureGraph 自身决定哪些节点可参与验证以及如何 revalidate。
    pub fn perform_validation_cycle(&mut self, update_manager: &mut dyn UpdateManager) {
        loop {
            let block_ids = update_manager.drain_invalid_blocks();
            if block_ids.is_empty() {
                break;
            }

            for block_id in &block_ids {
                self.mark_validation_path_invalid(*block_id);
            }

            let mut validation_roots: Vec<BlockId> = block_ids
                .into_iter()
                .filter_map(|block_id| self.validation_root(block_id))
                .collect();
            validation_roots.sort_by_key(|id| self.block_depth(*id).unwrap_or(usize::MAX));
            validation_roots.dedup();

            for root_id in validation_roots {
                self.revalidate_with_update(update_manager, root_id);
            }
        }
    }

    fn validation_root(&self, block_id: BlockId) -> Option<BlockId> {
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
        update_manager: &mut dyn UpdateManager,
        container_id: BlockId,
    ) {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return;
        }
        if !self.is_effectively_visible(container_id) {
            return;
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|b| b.layout.manager.take());

        if let Some(layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut layout_context = ValidationLayoutContext {
                    graph: self,
                    update_manager,
                };
                layout_manager.layout(container_id, &mut layout_context);
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            if let Err(payload) = result {
                std::panic::resume_unwind(payload);
            }
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        self.revalidate_children_with_update(update_manager, container_id);
        if let Some(block) = self.blocks.get_mut(container_id) {
            Updatable::validate(&mut *block.figure);
            block.is_valid = true;
        }
    }

    /// 递归验证子容器的布局
    fn revalidate_children_with_update(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        parent_id: BlockId,
    ) {
        // 先收集子元素 ID，避免在迭代过程中同时持有不可变和可变引用
        let children: Vec<BlockId> = self
            .blocks
            .get(parent_id)
            .map(|b| b.children.clone())
            .unwrap_or_default();

        for child_id in children {
            self.revalidate_with_update(update_manager, child_id);
        }
    }

    /// Immediately validates a subtree while preserving UpdateManager damage.
    ///
    /// This is the Draw2D `Figure.validate()` equivalent used by coordinated
    /// operations such as `ZoomManager`: scale invalidation is resolved before
    /// the new viewport location is applied.
    pub fn validate_with_update(
        &mut self,
        update_manager: &mut dyn UpdateManager,
        container_id: BlockId,
    ) {
        self.revalidate_with_update(update_manager, container_id);
    }

    /// 立即验证指定子树，不产生 damage。
    ///
    /// 该入口用于初始场景构建；运行时更新应通过 `mark_invalid` 和
    /// `UpdateManager::perform_update` 执行完整事务。
    pub fn revalidate(&mut self, container_id: BlockId) {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return;
        }
        if !self.is_effectively_visible(container_id) || !self.is_effectively_enabled(container_id)
        {
            return;
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|block| block.layout.manager.take());
        if let Some(layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                layout_manager.layout(container_id, self);
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            if let Err(payload) = result {
                std::panic::resume_unwind(payload);
            }
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
            self.revalidate(child_id);
        }
        if let Some(block) = self.blocks.get_mut(container_id) {
            Updatable::validate(&mut *block.figure);
            block.is_valid = true;
        }
    }

    /// 获取子元素 ID 列表
    #[allow(dead_code)]
    fn get_children_ids(&self, parent_id: BlockId) -> Vec<BlockId> {
        self.blocks
            .get(parent_id)
            .map(|b| b.children.clone())
            .unwrap_or_default()
    }

    /// 重新验证布局（兼容旧 API）
    ///
    /// 如果布局无效则重新计算。
    /// 使用内容块作为根容器。
    pub fn revalidate_with_bounds(&mut self, container_bounds: Rectangle) {
        if !self.is_layout_valid() {
            self.apply_layout(container_bounds);
            self.validate();
        }
    }

    /// 检查布局是否有效
    pub fn is_layout_valid(&self) -> bool {
        self.blocks
            .get(self.contents.unwrap_or(self.root))
            .map(|block| block.is_valid)
            .unwrap_or(true)
    }

    /// 返回单个节点的 validation 状态。
    pub fn is_valid(&self, block_id: BlockId) -> bool {
        self.blocks
            .get(block_id)
            .is_some_and(|block| block.is_valid)
    }

    /// 计算节点首选尺寸。显式覆盖优先，其次委托容器 LayoutManager，最后回退到 Figure。
    pub fn preferred_size(
        &self,
        block_id: BlockId,
        w_hint: f64,
        h_hint: f64,
    ) -> Option<(f64, f64)> {
        let block = self.blocks.get(block_id)?;
        let (w_hint, h_hint) = block.figure.layout_size_hints(w_hint, h_hint);
        if let Some(size) = block.preferred_size {
            return Some(block.figure.project_preferred_size(size));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let size = layout.get_preferred_size(block_id, w_hint, h_hint, self);
            return Some(block.figure.project_preferred_size(size));
        }
        Some(block.figure.preferred_size())
    }

    /// 计算节点最小尺寸。显式覆盖优先，其次委托容器 LayoutManager，最后回退到 Figure。
    pub fn minimum_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> Option<(f64, f64)> {
        let block = self.blocks.get(block_id)?;
        let (w_hint, h_hint) = block.figure.layout_size_hints(w_hint, h_hint);
        if let Some(size) = block.minimum_size {
            return Some(block.figure.project_minimum_size(size));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let size = layout.get_minimum_size(block_id, w_hint, h_hint, self);
            return Some(block.figure.project_minimum_size(size));
        }
        Some(block.figure.minimum_size())
    }

    /// 返回节点最大尺寸。显式覆盖优先，否则回退到 Figure。
    pub fn maximum_size(&self, block_id: BlockId) -> Option<(f64, f64)> {
        let block = self.blocks.get(block_id)?;
        Some(
            block
                .maximum_size
                .unwrap_or_else(|| block.figure.maximum_size()),
        )
    }

    pub fn set_preferred_size(&mut self, block_id: BlockId, size: Option<(f64, f64)>) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.preferred_size == size {
            return false;
        }
        block.preferred_size = size;
        self.mark_validation_path_invalid(block_id);
        true
    }

    pub fn set_minimum_size(&mut self, block_id: BlockId, size: Option<(f64, f64)>) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.minimum_size == size {
            return false;
        }
        block.minimum_size = size;
        self.mark_validation_path_invalid(block_id);
        true
    }

    pub fn set_maximum_size(&mut self, block_id: BlockId, size: Option<(f64, f64)>) -> bool {
        let Some(block) = self.blocks.get_mut(block_id) else {
            return false;
        };
        if block.maximum_size == size {
            return false;
        }
        block.maximum_size = size;
        self.mark_validation_path_invalid(block_id);
        true
    }

    /// 按矩形选择
    pub fn select_by_rect(&mut self, rect: Rectangle) {
        self.selected.clear();

        // 收集需要选中的 ID
        let mut to_select: Vec<BlockId> = Vec::new();
        let mut stack = vec![self.root];
        while let Some(node_id) = stack.pop() {
            if let Some(block) = self.blocks.get(node_id) {
                if !block.is_visible {
                    continue;
                }

                // 先处理子节点
                for &child_id in block.children.iter().rev() {
                    stack.push(child_id);
                }

                // 检查矩形相交
                let bounds = block.figure_bounds();
                if rect_intersects(&rect, &bounds) {
                    to_select.push(node_id);
                }
            }
        }

        // 设置选中状态
        self.selected.extend(to_select);
    }

    /// 选择单个块
    #[allow(clippy::collapsible_if)]
    pub fn select_single(&mut self, block_id: Option<BlockId>) {
        let mut changed = Vec::new();
        for id in self.blocks.keys() {
            let selected = Some(id) == block_id;
            let was_selected = self.selected.contains(&id);
            if was_selected != selected {
                changed.push((id, was_selected, selected));
            }
        }
        self.selected.clear();
        self.selected
            .extend(block_id.filter(|id| self.blocks.contains_key(*id)));
        for (id, old_value, new_value) in changed {
            self.emit_property_event(PropertyChangeEvent {
                block_id: id,
                property: "selected",
                old_value: PropertyValue::Bool(old_value),
                new_value: PropertyValue::Bool(new_value),
            });
        }
    }

    /// 设置选中状态
    pub fn set_selected(&mut self, block_id: Option<BlockId>) {
        self.select_single(block_id);
    }

    /// 获取当前选中的块 ID
    pub fn selected_block(&self) -> Option<BlockId> {
        self.selected.iter().next().copied()
    }

    /// 命中测试
    ///
    /// 检测指定点是否命中任意图形，返回从根到目标的路径。
    /// 使用深度优先遍历（逆序子节点，确保先命中最上层的图形）。
    ///
    /// # 坐标语义
    ///
    /// `point` 必须处于 logical surface domain。遍历时逐边逆变换到
    /// node-local 和 child content domain。
    ///
    /// # 参数
    ///
    /// - `point`: 待检测的坐标（与入口节点同域）
    ///
    /// # 返回
    ///
    /// Some((target, path)) 其中 target 是最底层命中的图形，path 是从根到目标的路径
    /// None 表示未命中任何图形
    pub fn hit_test(&self, point: (f64, f64)) -> Option<(BlockId, Vec<BlockId>)> {
        let start_id = self.contents.unwrap_or(self.root);
        let mut path = Vec::new();
        self.hit_test_from(start_id, point, &mut path)
    }

    /// 简单的命中测试
    ///
    /// 只返回第一个命中的块 ID，不包含路径。
    pub fn hit_test_simple(&self, point: (f64, f64)) -> Option<BlockId> {
        self.hit_test(point).map(|(target, _)| target)
    }

    pub fn find_mouse_event_target_at(&self, x: f64, y: f64) -> Option<BlockId> {
        self.find_mouse_event_target_from(self.contents.unwrap_or(self.root), (x, y))
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
        let scene_ref = FigureGraphRenderRef {
            blocks: &self.blocks,
            selected: &self.selected,
        };
        let mut renderer = FigureRenderer::new(&scene_ref, gc);
        renderer.render(start_id);
    }

    // ========== 调试验证方法 ==========

    /// 打印场景图树结构（用于调试）
    ///
    /// 使用 `eprintln!` 输出到 stderr，格式示例：
    /// ```text
    /// V BlockId(0x1): Figure bounds=(0,0,100,100)
    ///   V BlockId(0x2): RectangleFigure bounds=(10,10,50,50)
    ///   H BlockId(0x3): RectangleFigure bounds=(50,50,50,50)  // 不可见
    /// ```
    #[cfg(feature = "debug_render")]
    pub fn print_tree(&self) {
        eprintln!("\n========== 场景图结构 ==========");
        self.print_block(self.root, 0);
        eprintln!("=================================\n");
    }

    /// 递归打印单个块（内部使用）
    #[cfg(feature = "debug_render")]
    fn print_block(&self, block_id: BlockId, depth: usize) {
        let indent = "  ".repeat(depth);
        if let Some(block) = self.blocks.get(block_id) {
            let bounds = block.figure_bounds();
            let visibility = if block.is_visible { "V" } else { "H" };
            let selected = if self.selected.contains(&block_id) {
                " *"
            } else {
                ""
            };
            eprintln!(
                "{}{} {:?}: {} bounds=({:.0},{:.0},{:.0},{:.0}){}",
                indent,
                visibility,
                block_id,
                block.figure.name(),
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                selected
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
    pub fn get_block(&self, id: BlockId) -> Option<&FigureBlock> {
        self.blocks.get(id)
    }

    pub(crate) fn block(&self, id: BlockId) -> Option<&FigureBlock> {
        self.blocks.get(id)
    }

    /// 返回指定父节点的 child 顺序。
    ///
    /// 顺序与 Draw2D 一致：数组靠前的 child 先绘制，靠后的 child 后绘制并位于更高 z-order。
    pub fn child_order(&self, parent_id: BlockId) -> Option<Vec<BlockId>> {
        self.blocks
            .get(parent_id)
            .map(|block| block.children.clone())
    }

    /// Returns the direct parent of a block.
    pub fn parent_id(&self, block_id: BlockId) -> Option<BlockId> {
        self.blocks.get(block_id).and_then(|block| block.parent)
    }

    /// Returns whether a node is currently attached to this tree's root.
    pub fn is_attached(&self, block_id: BlockId) -> bool {
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
    pub fn child_z_index(&self, parent_id: BlockId, child_id: BlockId) -> Option<usize> {
        self.blocks
            .get(parent_id)?
            .children
            .iter()
            .position(|&id| id == child_id)
    }

    /// 将直接 child 移动到指定 z-order index。
    ///
    /// `index == 0` 表示最底层；`index == children.len() - 1` 表示最顶层。
    pub fn move_child_to_index(
        &mut self,
        parent_id: BlockId,
        child_id: BlockId,
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
    pub fn bring_child_to_front(&mut self, parent_id: BlockId, child_id: BlockId) -> bool {
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
    pub fn send_child_to_back(&mut self, parent_id: BlockId, child_id: BlockId) -> bool {
        self.move_child_to_index(parent_id, child_id, 0)
    }

    /// 获取指定块的 Figure bounds。
    pub fn figure_bounds(&self, id: BlockId) -> Option<Rectangle> {
        self.blocks.get(id).map(FigureBlock::figure_bounds)
    }

    /// 返回节点从 FigureGraph 根节点开始计算的深度。
    pub fn block_depth(&self, id: BlockId) -> Option<usize> {
        self.blocks.get(id).map(|block| block.depth)
    }

    /// 返回节点自身的本地可见性标志。
    pub fn is_visible(&self, id: BlockId) -> bool {
        self.blocks
            .get(id)
            .map(|block| block.is_visible)
            .unwrap_or(false)
    }

    /// 返回节点自身的本地启用标志。
    pub fn is_enabled(&self, id: BlockId) -> bool {
        self.blocks
            .get(id)
            .map(|block| block.is_enabled)
            .unwrap_or(false)
    }

    pub fn is_opaque(&self, id: BlockId) -> bool {
        self.blocks.get(id).is_some_and(|block| block.is_opaque)
    }

    pub fn insets(&self, id: BlockId) -> Option<(f64, f64, f64, f64)> {
        self.blocks.get(id).map(|block| block.insets)
    }

    pub fn style_override(&self, id: BlockId) -> Option<StyleOverride> {
        self.blocks.get(id).map(|block| block.style)
    }

    pub fn inherited_style(&self, id: BlockId) -> Option<StyleOverride> {
        self.blocks.get(id)?;
        let mut result = StyleOverride::default();
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = self.blocks.get(node_id)?;
            if result.foreground.is_none() {
                result.foreground = node.style.foreground;
            }
            if result.background.is_none() {
                result.background = node.style.background;
            }
            if result.alpha.is_none() {
                result.alpha = node.style.alpha;
            }
            current = node.parent;
        }
        Some(result)
    }

    pub fn set_insets(&mut self, id: BlockId, insets: (f64, f64, f64, f64)) -> bool {
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

    pub fn set_opaque(&mut self, id: BlockId, opaque: bool) -> bool {
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

    pub fn set_style_override(&mut self, id: BlockId, style: StyleOverride) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.style == style {
            return false;
        }
        block.style = style;
        self.notify_block_changed(id);
        true
    }

    /// 返回节点沿父链传播后的有效可见性。
    pub fn is_effectively_visible(&self, id: BlockId) -> bool {
        self.effective_flag_from(id, |block| block.is_visible)
    }

    /// 返回节点沿父链传播后的有效启用状态。
    pub fn is_effectively_enabled(&self, id: BlockId) -> bool {
        self.effective_flag_from(id, |block| block.is_enabled)
    }

    /// 设置块可见性。
    pub fn set_visible(&mut self, id: BlockId, visible: bool) -> bool {
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

        if !visible {
            self.clear_selection_for_subtree(id);
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
        update_manager: &mut dyn UpdateManager,
        id: BlockId,
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
    pub fn set_enabled(&mut self, id: BlockId, enabled: bool) -> bool {
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

        if !enabled {
            self.clear_selection_for_subtree(id);
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
        update_manager: &mut dyn UpdateManager,
        id: BlockId,
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
        block_id: BlockId,
        layout_manager: Box<dyn LayoutManager>,
    ) {
        if let Some(block) = self.blocks.get_mut(block_id) {
            block.layout.manager = Some(layout_manager);
        }
        self.mark_validation_path_invalid(block_id);
    }

    /// 获取指定块的布局管理器
    pub fn get_block_layout_manager(&self, block_id: BlockId) -> Option<&dyn LayoutManager> {
        self.blocks
            .get(block_id)
            .and_then(|b| b.layout.manager.as_deref())
    }

    /// 设置父容器施加给直接子节点的布局约束。
    pub fn set_constraint<C>(&mut self, child_id: BlockId, constraint: C) -> bool
    where
        C: LayoutConstraint,
    {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };
        parent
            .layout
            .constraints
            .insert(child_id, Box::new(constraint));
        self.mark_validation_path_invalid(parent_id);
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ConstraintChanged,
            container_id: parent_id,
            child_id: Some(child_id),
        });
        true
    }

    /// 获取指定类型的布局约束。
    pub fn get_constraint<C>(&self, child_id: BlockId) -> Option<&C>
    where
        C: LayoutConstraint,
    {
        self.constraint(child_id)?.as_any().downcast_ref::<C>()
    }

    /// 移除父容器为直接子节点保存的布局约束。
    pub fn remove_constraint(&mut self, child_id: BlockId) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let removed = self
            .blocks
            .get_mut(parent_id)
            .and_then(|parent| parent.layout.constraints.remove(&child_id))
            .is_some();
        if removed {
            self.mark_validation_path_invalid(parent_id);
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::ConstraintChanged,
                container_id: parent_id,
                child_id: Some(child_id),
            });
        }
        removed
    }

    fn constraint(&self, child_id: BlockId) -> Option<&dyn LayoutConstraint> {
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

    pub fn is_selected(&self, id: BlockId) -> bool {
        self.selected.contains(&id)
    }

    /// 应用布局
    ///
    /// 根据布局管理器重新计算子元素的位置。
    /// 注意：当前实现为简化版本。
    pub fn apply_layout(&mut self, container_bounds: Rectangle) {
        // TODO: 完整的布局实现需要基于约束系统
        // 当前简化实现：不做任何布局，子元素保持原位
        let _ = container_bounds;
    }

    /// 计算布局大小
    ///
    /// 返回容器的首选大小。
    pub fn compute_layout_size(&self, container_bounds: Rectangle) -> (f64, f64) {
        // TODO: 完整的布局实现需要基于约束系统
        // 当前简化实现：返回容器大小
        (container_bounds.width, container_bounds.height)
    }

    // ========== 坐标变换方法 ==========

    /// 原始平移（对应 draw2d: primTranslate）
    ///
    /// Moves one node in its parent content domain.
    ///
    /// Descendant bounds remain unchanged. Their projected positions change
    /// through the shared parent transform.
    pub fn prim_translate(&mut self, block_id: BlockId, dx: f64, dy: f64) {
        let Some((old_bounds, new_bounds, has_children)) =
            self.blocks.get_mut(block_id).map(|block| {
                let old_bounds = block.figure_bounds();
                let new_bounds = Rectangle::new(
                    old_bounds.x + dx,
                    old_bounds.y + dy,
                    old_bounds.width,
                    old_bounds.height,
                );
                block.set_figure_bounds(new_bounds);
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
    }

    fn emit_ancestor_moved(&mut self, ancestor_id: BlockId) {
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
    pub fn set_bounds(&mut self, block_id: BlockId, x: f64, y: f64, width: f64, height: f64) {
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
            block.set_figure_bounds(Rectangle::new(x, y, width, height));
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
        update_manager: &mut dyn UpdateManager,
        block_id: BlockId,
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
        }

        if visible {
            self.repaint(update_manager, block_id, None);
        }

        true
    }

    fn erase(
        &self,
        update_manager: &mut dyn UpdateManager,
        block_id: BlockId,
        old_bounds: Rectangle,
        mut old_visual_bounds: Rectangle,
        parent_id: Option<BlockId>,
    ) {
        let Some(parent_id) = parent_id else {
            return;
        };
        if !self.blocks.contains_key(block_id) || !self.blocks.contains_key(parent_id) {
            return;
        }

        old_visual_bounds.translate(old_bounds.x, old_bounds.y);
        if let Some(parent) = self.blocks.get(parent_id) {
            parent
                .figure
                .child_transform()
                .apply_to(&mut old_visual_bounds);
        }
        update_manager.add_dirty_region(parent_id, old_visual_bounds);
    }

    /// 将 node-local 几何转换到 logical surface domain。
    pub fn translate_to_absolute_mut<T: Translatable>(&self, block_id: BlockId, t: &mut T) {
        if let Some(transform) = self.local_to_surface_transform(block_id) {
            t.transform(transform);
        }
    }

    /// 将 node-local 几何转换到 parent content domain。
    pub fn translate_to_parent<T: Translatable>(&self, block_id: BlockId, t: &mut T) {
        if let Some(block) = self.blocks.get(block_id) {
            let bounds = block.figure_bounds();
            t.transform(novadraw_geometry::Affine2D::from_translation(
                bounds.x, bounds.y,
            ));
        }
    }

    /// 将 parent content 几何转换到 node-local domain。
    pub fn translate_from_parent<T: Translatable>(&self, block_id: BlockId, t: &mut T) {
        if let Some(block) = self.blocks.get(block_id) {
            let bounds = block.figure_bounds();
            t.transform(novadraw_geometry::Affine2D::from_translation(
                -bounds.x, -bounds.y,
            ));
        }
    }

    /// 将 logical surface 几何转换到 node-local domain。
    pub fn translate_to_relative<T: Translatable>(&self, block_id: BlockId, t: &mut T) -> bool {
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
        block_id: BlockId,
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
}

impl FigureGraph {
    fn mark_validation_path_invalid(&mut self, mut block_id: BlockId) {
        let mut invalidated = Vec::new();
        loop {
            let (parent, was_valid) = if let Some(block) = self.blocks.get_mut(block_id) {
                let was_valid = block.is_valid;
                block.is_valid = false;
                (block.parent, was_valid)
            } else {
                (None, false)
            };

            if !was_valid {
                break;
            }
            invalidated.push(block_id);
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

    pub(crate) fn invalid_block_ids(&self) -> Vec<BlockId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| (!block.is_valid).then_some(id))
            .collect()
    }

    fn hit_test_from(
        &self,
        block_id: BlockId,
        point: (f64, f64),
        path: &mut Vec<BlockId>,
    ) -> Option<(BlockId, Vec<BlockId>)> {
        let block = self.blocks.get(block_id)?;
        if !block.is_visible || !block.is_enabled {
            return None;
        }

        let mut local_point = point;
        self.translate_from_parent(block_id, &mut local_point);
        if !block
            .figure
            .precise_hit(local_point.0, local_point.1, block.figure_bounds())
        {
            return None;
        }

        path.push(block_id);
        let client_area = block.client_area();
        if !point_in_rect(local_point, &client_area) {
            let hit = Some((block_id, path.clone()));
            path.pop();
            return hit;
        }
        let mut child_point = local_point;
        if !block.child_transform().apply_inverse_to(&mut child_point) {
            let hit = Some((block_id, path.clone()));
            path.pop();
            return hit;
        }

        for &child_id in block.children.iter().rev() {
            if let Some(hit) = self.hit_test_from(child_id, child_point, path) {
                return Some(hit);
            }
        }

        let hit = Some((block_id, path.clone()));
        path.pop();
        hit
    }

    fn find_mouse_event_target_from(
        &self,
        block_id: BlockId,
        point: (f64, f64),
    ) -> Option<BlockId> {
        let block = self.blocks.get(block_id)?;
        if !block.is_visible || !block.is_enabled {
            return None;
        }

        let mut local_point = point;
        self.translate_from_parent(block_id, &mut local_point);
        let contains =
            block
                .figure
                .precise_hit(local_point.0, local_point.1, block.figure_bounds());
        if !contains {
            return None;
        }

        let client_area = block.client_area();
        if !point_in_rect(local_point, &client_area) {
            return block
                .figure
                .event_handler()
                .is_some_and(|handler| handler.wants_mouse_events())
                .then_some(block_id);
        }
        let mut child_point = local_point;
        if !block.child_transform().apply_inverse_to(&mut child_point) {
            return block
                .figure
                .event_handler()
                .is_some_and(|handler| handler.wants_mouse_events())
                .then_some(block_id);
        }

        for &child_id in block.children.iter().rev() {
            if let Some(target) = self.find_mouse_event_target_from(child_id, child_point) {
                return Some(target);
            }
        }

        block
            .figure
            .event_handler()
            .is_some_and(|handler| handler.wants_mouse_events())
            .then_some(block_id)
    }

    fn clear_selection_for_subtree(&mut self, subtree_root: BlockId) {
        let descendants: Vec<BlockId> = self
            .blocks
            .keys()
            .filter(|&id| self.is_in_subtree(id, subtree_root))
            .collect();
        let deselected: Vec<_> = descendants
            .into_iter()
            .filter(|id| self.selected.remove(id))
            .collect();
        for block_id in deselected {
            self.emit_property_event(PropertyChangeEvent {
                block_id,
                property: "selected",
                old_value: PropertyValue::Bool(true),
                new_value: PropertyValue::Bool(false),
            });
        }
    }

    fn is_in_subtree(&self, block_id: BlockId, subtree_root: BlockId) -> bool {
        let mut current = Some(block_id);
        while let Some(id) = current {
            if id == subtree_root {
                return true;
            }
            current = self.blocks.get(id).and_then(|block| block.parent);
        }
        false
    }

    fn effective_flag_from(
        &self,
        mut block_id: BlockId,
        local_flag: fn(&FigureBlock) -> bool,
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

struct ValidationLayoutContext<'a> {
    graph: &'a mut FigureGraph,
    update_manager: &'a mut dyn UpdateManager,
}

impl super::layout::LayoutContext for ValidationLayoutContext<'_> {
    fn get_children(&self, parent_id: BlockId) -> Vec<(BlockId, Rectangle)> {
        <FigureGraph as super::layout::LayoutContext>::get_children(self.graph, parent_id)
    }

    fn get_constraint(&self, child_id: BlockId) -> Option<&dyn LayoutConstraint> {
        self.graph.constraint(child_id)
    }

    fn get_preferred_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.graph
            .preferred_size(block_id, w_hint, h_hint)
            .unwrap_or((0.0, 0.0))
    }

    fn get_minimum_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.graph
            .minimum_size(block_id, w_hint, h_hint)
            .unwrap_or((0.0, 0.0))
    }

    fn get_maximum_size(&self, block_id: BlockId) -> (f64, f64) {
        self.graph
            .maximum_size(block_id)
            .unwrap_or((f64::INFINITY, f64::INFINITY))
    }

    fn set_child_bounds(&mut self, child_id: BlockId, bounds: Rectangle) {
        self.graph.set_bounds_with_update(
            self.update_manager,
            child_id,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        );
    }

    fn set_child_visible(&mut self, child_id: BlockId, visible: bool) {
        self.graph
            .set_visible_with_update(self.update_manager, child_id, visible);
    }

    fn get_container_bounds(&self, container_id: BlockId) -> Rectangle {
        <FigureGraph as super::layout::LayoutContext>::get_container_bounds(
            self.graph,
            container_id,
        )
    }
}

impl super::layout::LayoutContext for FigureGraph {
    fn get_children(&self, parent_id: BlockId) -> Vec<(BlockId, Rectangle)> {
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

    fn get_constraint(&self, child_id: BlockId) -> Option<&dyn LayoutConstraint> {
        self.constraint(child_id)
    }

    fn get_preferred_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.preferred_size(block_id, w_hint, h_hint)
            .unwrap_or((0.0, 0.0))
    }

    fn get_minimum_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.minimum_size(block_id, w_hint, h_hint)
            .unwrap_or((0.0, 0.0))
    }

    fn get_maximum_size(&self, block_id: BlockId) -> (f64, f64) {
        self.maximum_size(block_id)
            .unwrap_or((f64::INFINITY, f64::INFINITY))
    }

    fn set_child_bounds(&mut self, child_id: BlockId, bounds: Rectangle) {
        let Some(old_bounds) = self.figure_bounds(child_id) else {
            return;
        };
        self.set_bounds(child_id, bounds.x, bounds.y, bounds.width, bounds.height);
        if (old_bounds.width != bounds.width || old_bounds.height != bounds.height)
            && let Some(child) = self.blocks.get_mut(child_id)
        {
            child.is_valid = false;
        }
    }

    fn set_child_visible(&mut self, child_id: BlockId, visible: bool) {
        self.set_visible(child_id, visible);
    }

    fn get_container_bounds(&self, container_id: BlockId) -> Rectangle {
        if let Some(block) = self.blocks.get(container_id) {
            block.client_area()
        } else {
            Rectangle::new(0.0, 0.0, 0.0, 0.0)
        }
    }
}

impl Default for FigureGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::super::figure::{Bounded, ChildClippingStrategy, RectangleFigure, Shape, Updatable};
    use crate::{
        BlockId, EllipseFigure, Figure, FigureEvent, FigureEventHandler, FigureGraph,
        FigureLifecycle, LineBorder, NotificationEffect, PolygonFigure, PolylineFigure, Rectangle,
        RootFigure, RoundedRectangleFigure, ScalableLayeredPaneFigure, TriangleFigure,
        ViewportFigure,
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

    impl Updatable for TestCoordinateRootFigure {
        fn validate(&mut self) {}
        fn invalidate(&mut self) {}
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

    impl Updatable for OverflowPaintFigure {
        fn validate(&mut self) {}
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
        Attached(BlockId),
        Detached(BlockId),
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

    impl Updatable for LifecycleRecordingFigure {
        fn validate(&mut self) {}
    }

    impl Figure for LifecycleRecordingFigure {
        fn lifecycle(&mut self) -> Option<&mut dyn FigureLifecycle> {
            Some(self)
        }
    }

    impl FigureLifecycle for LifecycleRecordingFigure {
        fn on_attached(&mut self, parent_id: BlockId) {
            self.events
                .lock()
                .unwrap()
                .push(LifecycleEvent::Attached(parent_id));
        }

        fn on_detached(&mut self, parent_id: BlockId) {
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

    impl Updatable for TestFigureWithInsets {
        fn validate(&mut self) {}
        fn invalidate(&mut self) {}
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

    impl Updatable for TestInteractiveFigure {
        fn validate(&mut self) {}
        fn invalidate(&mut self) {}
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
                    fn paint_figure(&self, gc: &mut NdCanvas) {
                        Shape::paint_figure(self, gc);
                    }
                }
            )+
        };
    }

    impl_test_shape_figure!(
        TestCoordinateRootFigure,
        OverflowPaintFigure,
        TestFigureWithInsets,
    );

    impl Figure for TestInteractiveFigure {
        fn paint_figure(&self, gc: &mut NdCanvas) {
            Shape::paint_figure(self, gc);
        }

        fn event_handler(&self) -> Option<&dyn FigureEventHandler> {
            Some(self)
        }
    }

    struct AlphaStateFigure {
        bounds: Rectangle,
        alpha: Option<f64>,
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

    impl Updatable for AlphaStateFigure {
        fn validate(&mut self) {}
    }

    impl Figure for AlphaStateFigure {
        fn init_properties(&self, gc: &mut NdCanvas) {
            if let Some(alpha) = self.alpha {
                gc.global_alpha(alpha);
            }
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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();
        scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        assert_eq!(scene.find_mouse_event_target_at(10.0, 10.0), None);
    }

    #[test]
    fn test_find_mouse_event_target_at_prefers_deepest_interactive_figure() {
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
    fn migrate_legacy_bounds_converts_shared_domains_and_preserves_coordinate_roots() {
        let mut scene = FigureGraph::new();
        let contents =
            scene.set_contents(Box::new(RectangleFigure::new(100.0, 50.0, 500.0, 400.0)));
        let parent = scene.add_child_to(
            contents,
            Box::new(TestFigureWithInsets::new(
                130.0,
                80.0,
                200.0,
                150.0,
                (5.0, 7.0, 0.0, 0.0),
            )),
        );
        let legacy_root = scene.add_child_to(
            parent,
            Box::new(RectangleFigure::new(150.0, 100.0, 80.0, 60.0)),
        );
        let child = scene.add_child_to(
            legacy_root,
            Box::new(RectangleFigure::new(10.0, 15.0, 20.0, 10.0)),
        );

        let changed = scene
            .migrate_legacy_bounds_to_parent_local(&std::collections::HashSet::from([legacy_root]));

        assert_eq!(changed, 2);
        assert_eq!(
            scene.figure_bounds(parent),
            Some(Rectangle::new(30.0, 30.0, 200.0, 150.0))
        );
        assert_eq!(
            scene.figure_bounds(legacy_root),
            Some(Rectangle::new(13.0, 15.0, 80.0, 60.0))
        );
        assert_eq!(
            scene.figure_bounds(child),
            Some(Rectangle::new(10.0, 15.0, 20.0, 10.0))
        );
    }

    #[test]
    fn moved_figure_damage_uses_old_and_new_projected_visual_bounds() {
        let mut scene = FigureGraph::new();
        let contents = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 300.0, 240.0)));
        let figure = scene.add_child_to(
            contents,
            Box::new(OverflowPaintFigure::new(
                Rectangle::new(50.0, 40.0, 20.0, 20.0),
                Rectangle::new(-5.0, -6.0, 30.0, 32.0),
            )),
        );
        let mut updates = crate::SceneUpdateManager::new();

        assert!(scene.set_bounds_with_update(&mut updates, figure, 70.0, 60.0, 20.0, 20.0,));

        let canvas = scene.perform_update(&mut updates);
        assert_eq!(
            canvas.damage().union(),
            Some(Rectangle::new(45.0, 34.0, 50.0, 52.0))
        );
    }

    #[test]
    fn test_border_insets_define_client_area_clip_for_children() {
        let mut scene = FigureGraph::new();

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
            signatures.contains(&RenderSignature::StrokeRect([21.0, 11.0, 79.0, 69.0])),
            "border must render in its inset-adjusted bounds"
        );

        let child_fill_index = signatures
            .iter()
            .position(|signature| *signature == RenderSignature::FillRect([0.0, 0.0, 20.0, 20.0]))
            .expect("child fill must be rendered under parent clientArea clip");
        let parent_border_index = signatures
            .iter()
            .position(|signature| {
                *signature == RenderSignature::StrokeRect([21.0, 11.0, 79.0, 69.0])
            })
            .expect("parent border must be rendered");
        assert!(
            parent_border_index > child_fill_index,
            "border must render after children"
        );
    }

    #[test]
    fn test_paint_clip_and_hit_test_share_border_inset_client_area() {
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();

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
        let mut scene = FigureGraph::new();
        let parent_id = scene.set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 100.0, 100.0)
                .with_child_clipping_strategy(ChildClippingStrategy::DoNotClipChildBounds),
        ));
        scene.add_child_to(
            parent_id,
            Box::new(AlphaStateFigure {
                bounds: Rectangle::new(0.0, 0.0, 10.0, 10.0),
                alpha: Some(0.25),
            }),
        );
        scene.add_child_to(
            parent_id,
            Box::new(AlphaStateFigure {
                bounds: Rectangle::new(20.0, 0.0, 10.0, 10.0),
                alpha: None,
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
            let mut scene = FigureGraph::new();
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
        let mut scene = FigureGraph::new();

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
}
