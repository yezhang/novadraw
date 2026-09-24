//! Notification effects - 通知 effect
//!
//! 这里刻意不实现完整 listener 框架，只定义最核心的通知语义和 effect 队列。
//!
//! 设计来源：
//!
//! - draw2d：保留 `figureMoved`、`coordinateSystemChanged`、UpdateListener 等语义分层
//! - Zed：状态变化和 typed event 分离，通知先进入 effect 队列，等待事务边界 flush

use novadraw_core::Color;
use novadraw_geometry::{Dimension, Point, Rectangle};
use novadraw_render::{DamageMode, FrameId, RenderOutcome};

use crate::graph::FigureId;
use crate::style::CursorIcon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId {
    namespace: crate::RuntimeNamespace,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerScope {
    Runtime,
    Figure(FigureId),
}

impl ListenerId {
    pub(crate) fn new(namespace: crate::RuntimeNamespace, sequence: u64) -> Self {
        Self {
            namespace,
            sequence,
        }
    }

    pub fn namespace(self) -> crate::RuntimeNamespace {
        self.namespace
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListenerDirective {
    #[default]
    Keep,
    Remove,
}

/// Update Event - 更新事件
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateEvent {
    /// 验证开始前
    Validating,
    /// 验证完成后
    Validated,
    /// 重绘开始
    Painting { damage: Rectangle },
    /// 重绘完成
    Painted { damage: Rectangle },
    /// RenderSubmission 已在稳定事务边界生成。
    Prepared {
        frame_id: FrameId,
        damage: DamageMode,
    },
    /// 后端已返回该提交的处理结果。
    Submitted {
        frame_id: FrameId,
        outcome: RenderOutcome,
    },
}

/// Figure 语义事件
///
/// 对应 draw2d 中 Figure/Coordinate 相关 listener 的核心语义。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FigureEvent {
    /// Figure 的 bounds 发生变化。
    ///
    /// 对应 draw2d: `FigureListener.figureMoved(...)`。
    FigureMoved {
        figure_id: FigureId,
        old_bounds: Rectangle,
        new_bounds: Rectangle,
    },
    /// 当前 Figure 作为坐标根时，其局部坐标系统映射发生变化。
    ///
    /// 对应 draw2d: `CoordinateListener.coordinateSystemChanged(...)`。
    CoordinateSystemChanged {
        figure_id: FigureId,
        old_bounds: Rectangle,
        new_bounds: Rectangle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AncestorEventKind {
    Added,
    Moved,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AncestorEvent {
    pub kind: AncestorEventKind,
    pub figure_id: FigureId,
    pub parent_id: FigureId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Bool(bool),
    Number(f64),
    Color(Color),
    Cursor(CursorIcon),
    Point(Point),
    PointList(Vec<Point>),
    Size(Dimension),
    Rectangle(Rectangle),
    Text(String),
    Figure(Option<FigureId>),
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyChangeEvent {
    pub figure_id: FigureId,
    pub property: &'static str,
    pub old_value: PropertyValue,
    pub new_value: PropertyValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionEvent {
    pub figure_id: FigureId,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutEventKind {
    Invalidated,
    Started,
    Finished,
    ConstraintChanged,
    ChildRemoved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutEvent {
    pub kind: LayoutEventKind,
    pub container_id: FigureId,
    pub child_id: Option<FigureId>,
}

/// 通知 effect
///
/// `Notify` 表达“对象状态已变化”，不携带业务 payload。
/// `EmitFigure` / `EmitUpdate` 表达 typed 语义事件。
///
/// 该分层对应 Zed 的 `notify` / `emit` 分离，同时保留 draw2d 的 Figure/Update 语义。
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationEffect {
    /// 无 payload 的状态失效通知。
    Notify { figure_id: FigureId },
    /// Figure 层 typed event。
    EmitFigure(FigureEvent),
    /// UpdateManager 层 typed event。
    EmitUpdate(UpdateEvent),
    /// 祖先链变化。
    EmitAncestor(AncestorEvent),
    /// 通用属性变化。
    EmitProperty(PropertyChangeEvent),
    /// Button-like Figure activation.
    EmitAction(ActionEvent),
    /// 布局生命周期变化。
    EmitLayout(LayoutEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NotificationRecord {
    pub source_epoch: u64,
    pub sequence: u64,
    pub effect: NotificationEffect,
}

#[derive(Clone, Copy)]
pub struct StableSceneQuery<'a> {
    epoch: u64,
    tree: &'a crate::FigureTree,
}

impl<'a> StableSceneQuery<'a> {
    pub(crate) fn new(epoch: u64, tree: &'a crate::FigureTree) -> Self {
        Self { epoch, tree }
    }

    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    pub fn is_attached(self, figure: FigureId) -> bool {
        self.tree.is_attached(figure)
    }

    pub fn bounds(self, figure: FigureId) -> Option<Rectangle> {
        self.tree.figure_bounds(figure)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableQueryError {
    Faulted,
    NotStable { latest_stable_epoch: u64 },
}

impl std::fmt::Display for StableQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Faulted => formatter.write_str("Runtime is faulted"),
            Self::NotStable {
                latest_stable_epoch,
            } => write!(
                formatter,
                "Runtime has unpublished work after stable epoch {latest_stable_epoch}"
            ),
        }
    }
}

impl std::error::Error for StableQueryError {}

pub trait ObservationListener {
    fn observed(
        &self,
        record: &NotificationRecord,
        latest: StableSceneQuery<'_>,
    ) -> ListenerDirective;
}

/// 通知 effect 队列
///
/// 该队列是后续 listener/subscription 系统的最小核心。
/// 任何 Figure 或 UpdateManager 的变化都应先记录 effect，再由事务边界统一 drain/flush。
#[derive(Debug, Default, Clone)]
pub struct NotificationQueue {
    effects: Vec<NotificationEffect>,
}

impl NotificationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify(&mut self, figure_id: FigureId) {
        self.effects.push(NotificationEffect::Notify { figure_id });
    }

    pub fn emit_figure(&mut self, event: FigureEvent) {
        self.effects.push(NotificationEffect::EmitFigure(event));
    }

    pub fn emit_update(&mut self, event: UpdateEvent) {
        self.effects.push(NotificationEffect::EmitUpdate(event));
    }

    pub fn emit_ancestor(&mut self, event: AncestorEvent) {
        self.effects.push(NotificationEffect::EmitAncestor(event));
    }

    pub fn emit_property(&mut self, event: PropertyChangeEvent) {
        self.effects.push(NotificationEffect::EmitProperty(event));
    }

    pub fn emit_action(&mut self, event: ActionEvent) {
        self.effects.push(NotificationEffect::EmitAction(event));
    }

    pub fn emit_layout(&mut self, event: LayoutEvent) {
        self.effects.push(NotificationEffect::EmitLayout(event));
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn effects(&self) -> &[NotificationEffect] {
        &self.effects
    }

    pub fn extend(&mut self, effects: impl IntoIterator<Item = NotificationEffect>) {
        self.effects.extend(effects);
    }

    pub fn retain_semantic_effects(&mut self) {
        self.effects
            .retain(|effect| !matches!(effect, NotificationEffect::EmitUpdate(_)));
    }

    pub fn drain(&mut self) -> Vec<NotificationEffect> {
        self.effects.drain(..).collect()
    }
}

/// Update Listener trait
///
/// 监听场景图的更新和图形事件。
///
/// # 使用场景
///
/// - 调试：观察更新时机和区域
/// - 性能分析：统计更新频率和耗时
/// - 动画：协调多个视图的更新
/// - 视图同步：当 Figure 移动或坐标系统变化时更新视图状态
pub trait UpdateListener {
    /// 通知 Update 层事件（验证、重绘阶段变化）
    fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective;

    /// 通知 Figure 层事件（FigureMoved, CoordinateSystemChanged）
    fn on_figure_event(&self, event: FigureEvent) -> ListenerDirective;

    /// 通知 Figure 状态变化（Notify 语义）
    fn on_notify(&self, figure_id: FigureId) -> ListenerDirective;

    /// 检查是否为验证监听器
    fn as_validating_listener(&self) -> Option<&dyn ValidatingListener> {
        None
    }
}

/// Validating Listener - 验证监听器
///
/// 专门监听验证（布局）阶段的监听器。
pub trait ValidatingListener {
    /// 通知验证开始
    fn notify_validating(&self);

    /// 通知验证完成
    fn notify_validated(&self);
}

pub trait FigureListener {
    fn figure_moved(&self, event: FigureEvent) -> ListenerDirective;
}

pub trait CoordinateListener {
    fn coordinate_system_changed(&self, event: FigureEvent) -> ListenerDirective;
}

pub trait AncestorListener {
    fn ancestor_changed(&self, event: AncestorEvent) -> ListenerDirective;
}

pub trait PropertyChangeListener {
    fn property_changed(&self, event: &PropertyChangeEvent) -> ListenerDirective;
}

pub trait ActionListener {
    fn action_performed(&self, event: ActionEvent) -> ListenerDirective;
}

pub trait LayoutListener {
    fn layout_changed(&self, event: LayoutEvent) -> ListenerDirective;
}

/// No-op 实现
impl UpdateListener for () {
    fn on_update_event(&self, _event: UpdateEvent) -> ListenerDirective {
        ListenerDirective::Keep
    }

    fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
        ListenerDirective::Keep
    }

    fn on_notify(&self, _figure_id: FigureId) -> ListenerDirective {
        ListenerDirective::Keep
    }
}

impl ValidatingListener for () {
    fn notify_validating(&self) {}
    fn notify_validated(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::KeyData;

    fn figure_id(data: u64) -> FigureId {
        FigureId::from(KeyData::from_ffi(data))
    }

    #[test]
    fn test_notification_queue_separates_notify_and_typed_event() {
        let id = figure_id(1);
        let mut queue = NotificationQueue::new();

        queue.notify(id);
        queue.emit_figure(FigureEvent::CoordinateSystemChanged {
            figure_id: id,
            old_bounds: Rectangle::new(0.0, 0.0, 10.0, 10.0),
            new_bounds: Rectangle::new(5.0, 5.0, 10.0, 10.0),
        });

        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue.effects()[0],
            NotificationEffect::Notify { figure_id: id }
        );
        assert!(matches!(
            queue.effects()[1],
            NotificationEffect::EmitFigure(FigureEvent::CoordinateSystemChanged { .. })
        ));
    }

    #[test]
    fn test_notification_queue_drains_at_transaction_boundary() {
        let id = figure_id(1);
        let mut queue = NotificationQueue::new();
        queue.notify(id);

        let drained = queue.drain();

        assert_eq!(drained, vec![NotificationEffect::Notify { figure_id: id }]);
        assert!(queue.is_empty());
    }
}
