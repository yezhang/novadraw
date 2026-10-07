//! Scene Update Manager - 场景更新管理器
//!
//! 实现延迟批量更新机制，参考 Eclipse Draw2D 的 DeferredUpdateManager。
//!
//! # 核心功能
//!
//! 1. **脏区域（Dirty Region）跟踪**
//!    - 收集 repaint() 请求
//!    - 合并重叠区域，减少重绘次数
//!
//! 2. **失效块（Invalid Block）队列**
//!    - 收集 revalidate() 请求
//!    - 在重绘前先执行布局
//!
//! 3. **两阶段更新**
//!    - Phase 1: 布局失效的块
//!    - Phase 2: 合并并重绘脏区域

use crate::geometry::Rectangle;
use crate::render::NdCanvas;

use crate::ValidationError;
use crate::graph::FigureId;
use crate::runtime::update::listener::{
    ActionListener, AncestorListener, CoordinateListener, FigureListener, LayoutListener,
    ListenerDirective, ListenerId, ListenerScope, NotificationEffect, NotificationQueue,
    NotificationRecord, ObservationListener, PropertyChangeListener, StableSceneQuery, UpdateEvent,
    UpdateListener,
};
use crate::runtime::update::repair::{
    compute_damage_union, merge_dirty_region, prepare_damage_set,
};

/// Scene Update Manager
///
/// 场景图更新管理器，批量处理布局和重绘请求。
/// 参考 Eclipse Draw2D 的 DeferredUpdateManager 设计。
///
/// # 设计要点
///
/// - 脏区域使用 HashMap 合并，每个块最多一个脏区域
/// - 失效块使用 Vec 存储，支持重复添加（去重）
/// - 两阶段更新：先布局，再重绘
/// - 具体事务组件：与 Runtime 共同维护 validation 和 recording 顺序
///
/// # 与 draw2d 的差异
///
/// draw2d 的 DeferredUpdateManager 直接持有 root Figure 引用并调用其方法。
/// 本实现由 Runtime 持有 manager，并在事务执行时显式传入 FigureTree。
pub struct UpdateManager {
    namespace: crate::RuntimeNamespace,
    listener_owners: std::collections::HashMap<ListenerId, FigureId>,
    /// 脏区域映射：figure_id -> 脏区域
    pub(crate) dirty_regions: std::collections::HashMap<FigureId, Rectangle>,
    /// 几何或拓扑变更前投影并冻结的 logical-surface 脏区域。
    pub(crate) frozen_surface_regions: Vec<Rectangle>,
    /// 失效 Figure 队列
    pub(crate) invalid_figures: Vec<FigureId>,
    /// 是否有更新待处理
    pub(crate) update_queued: bool,
    pub(crate) updating: bool,
    notification_effects: NotificationQueue,
    animation_effect_cursor: usize,
    listeners: Vec<(ListenerId, Box<dyn UpdateListener>)>,
    figure_listeners: Vec<(ListenerId, Box<dyn FigureListener>)>,
    coordinate_listeners: Vec<(ListenerId, Box<dyn CoordinateListener>)>,
    ancestor_listeners: Vec<(ListenerId, Box<dyn AncestorListener>)>,
    property_listeners: Vec<(ListenerId, Box<dyn PropertyChangeListener>)>,
    action_listeners: Vec<(ListenerId, Box<dyn ActionListener>)>,
    layout_listeners: Vec<(ListenerId, Box<dyn LayoutListener>)>,
    observation_listeners: Vec<(ListenerId, Box<dyn ObservationListener>)>,
    next_listener_id: u64,
    next_notification_sequence: u64,
    publication_epoch: u64,
    last_validation_error: Option<ValidationError>,
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateManager {
    pub(crate) fn freeze_figure_damage(
        &mut self,
        tree: &crate::FigureTree,
        id: FigureId,
        local_region: Rectangle,
    ) -> bool {
        let Some(surface_region) = super::repair::propagate_damage_to_root(tree, id, local_region)
        else {
            return false;
        };
        self.add_frozen_surface_region(surface_region);
        true
    }

    pub(crate) fn freeze_removed_damage(
        &self,
        tree: &crate::FigureTree,
        ids: &[FigureId],
    ) -> Vec<Rectangle> {
        ids.iter()
            .flat_map(|id| {
                let visual = tree.node(*id).map(|node| node.visual_bounds());
                visual
                    .into_iter()
                    .chain(self.dirty_regions.get(id).copied())
                    .filter_map(|rect| super::repair::propagate_damage_to_root(tree, *id, rect))
            })
            .collect()
    }

    pub(crate) fn forget_figures(&mut self, ids: &std::collections::HashSet<FigureId>) {
        self.invalid_figures.retain(|id| !ids.contains(id));
        self.dirty_regions.retain(|id, _| !ids.contains(id));
    }

    pub(crate) fn set_listener_scope(&mut self, id: ListenerId, scope: ListenerScope) -> bool {
        let exists = self.listeners.iter().any(|(key, _)| *key == id)
            || self.figure_listeners.iter().any(|(key, _)| *key == id)
            || self.coordinate_listeners.iter().any(|(key, _)| *key == id)
            || self.ancestor_listeners.iter().any(|(key, _)| *key == id)
            || self.property_listeners.iter().any(|(key, _)| *key == id)
            || self.action_listeners.iter().any(|(key, _)| *key == id)
            || self.layout_listeners.iter().any(|(key, _)| *key == id)
            || self.observation_listeners.iter().any(|(key, _)| *key == id);
        if exists {
            match scope {
                ListenerScope::Runtime => {
                    self.listener_owners.remove(&id);
                }
                ListenerScope::Figure(owner) => {
                    self.listener_owners.insert(id, owner);
                }
            }
        }
        exists
    }

    pub(crate) fn retire_listeners(&mut self, ids: &std::collections::HashSet<FigureId>) -> Self {
        let mut retired = Self::with_namespace(self.namespace);
        let owners = &self.listener_owners;
        macro_rules! retire {
            ($field:ident) => {
                let (removed, kept) = std::mem::take(&mut self.$field)
                    .into_iter()
                    .partition(|(id, _)| owners.get(id).is_some_and(|owner| ids.contains(owner)));
                retired.$field = removed;
                self.$field = kept;
            };
        }
        retire!(listeners);
        retire!(figure_listeners);
        retire!(coordinate_listeners);
        retire!(ancestor_listeners);
        retire!(property_listeners);
        retire!(action_listeners);
        retire!(layout_listeners);
        retire!(observation_listeners);
        self.listener_owners.retain(|_, owner| !ids.contains(owner));
        retired
    }

    /// 创建新的场景更新管理器
    pub fn new() -> Self {
        Self::with_namespace(crate::RuntimeNamespace::new())
    }

    pub(crate) fn with_namespace(namespace: crate::RuntimeNamespace) -> Self {
        Self {
            namespace,
            listener_owners: std::collections::HashMap::new(),
            dirty_regions: std::collections::HashMap::new(),
            frozen_surface_regions: Vec::new(),
            invalid_figures: Vec::new(),
            update_queued: false,
            updating: false,
            notification_effects: NotificationQueue::new(),
            animation_effect_cursor: 0,
            listeners: Vec::new(),
            figure_listeners: Vec::new(),
            coordinate_listeners: Vec::new(),
            ancestor_listeners: Vec::new(),
            property_listeners: Vec::new(),
            action_listeners: Vec::new(),
            layout_listeners: Vec::new(),
            observation_listeners: Vec::new(),
            next_listener_id: 1,
            next_notification_sequence: 1,
            publication_epoch: 0,
            last_validation_error: None,
        }
    }

    /// 注册更新监听器
    pub fn add_listener(&mut self, listener: Box<dyn UpdateListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.listeners.push((id, listener));
        id
    }

    pub fn add_figure_listener(&mut self, listener: Box<dyn FigureListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.figure_listeners.push((id, listener));
        id
    }

    pub fn add_coordinate_listener(&mut self, listener: Box<dyn CoordinateListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.coordinate_listeners.push((id, listener));
        id
    }

    pub fn add_ancestor_listener(&mut self, listener: Box<dyn AncestorListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.ancestor_listeners.push((id, listener));
        id
    }

    pub fn add_property_listener(
        &mut self,
        listener: Box<dyn PropertyChangeListener>,
    ) -> ListenerId {
        let id = self.allocate_listener_id();
        self.property_listeners.push((id, listener));
        id
    }

    pub fn add_action_listener(&mut self, listener: Box<dyn ActionListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.action_listeners.push((id, listener));
        id
    }

    pub fn add_layout_listener(&mut self, listener: Box<dyn LayoutListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.layout_listeners.push((id, listener));
        id
    }

    pub fn add_observation_listener(
        &mut self,
        listener: Box<dyn ObservationListener>,
    ) -> ListenerId {
        let id = self.allocate_listener_id();
        self.observation_listeners.push((id, listener));
        id
    }

    pub fn remove_listener(&mut self, id: ListenerId) -> bool {
        self.listener_owners.remove(&id);
        remove_listener(&mut self.listeners, id)
            || remove_listener(&mut self.figure_listeners, id)
            || remove_listener(&mut self.coordinate_listeners, id)
            || remove_listener(&mut self.ancestor_listeners, id)
            || remove_listener(&mut self.property_listeners, id)
            || remove_listener(&mut self.action_listeners, id)
            || remove_listener(&mut self.layout_listeners, id)
            || remove_listener(&mut self.observation_listeners, id)
    }

    fn allocate_listener_id(&mut self) -> ListenerId {
        let id = ListenerId::new(self.namespace, self.next_listener_id);
        self.next_listener_id = self
            .next_listener_id
            .checked_add(1)
            .expect("listener id space exhausted");
        id
    }

    /// 向所有监听器分发 effect 队列中的事件
    fn dispatch_effects(
        &mut self,
        effects: &[NotificationEffect],
        graph: &crate::FigureTree,
        source_epoch: u64,
    ) {
        for effect in effects {
            let sequence = self.next_notification_sequence;
            self.next_notification_sequence = self
                .next_notification_sequence
                .checked_add(1)
                .expect("notification sequence space exhausted");
            let record = NotificationRecord {
                source_epoch,
                sequence,
                effect: effect.clone(),
            };
            let latest = StableSceneQuery::new(source_epoch, graph);
            self.observation_listeners.retain(|(_, listener)| {
                listener.observed(&record, latest) == ListenerDirective::Keep
            });
            match effect {
                NotificationEffect::Notify { figure_id } => {
                    self.listeners.retain(|(_, listener)| {
                        listener.on_notify(*figure_id) == ListenerDirective::Keep
                    });
                }
                NotificationEffect::EmitFigure(event) => {
                    self.listeners.retain(|(_, listener)| {
                        listener.on_figure_event(*event) == ListenerDirective::Keep
                    });
                    match event {
                        crate::FigureEvent::FigureMoved { .. } => {
                            self.figure_listeners.retain(|(_, listener)| {
                                listener.figure_moved(*event) == ListenerDirective::Keep
                            });
                        }
                        crate::FigureEvent::CoordinateSystemChanged { .. } => {
                            self.coordinate_listeners.retain(|(_, listener)| {
                                listener.coordinate_system_changed(*event)
                                    == ListenerDirective::Keep
                            });
                        }
                    }
                }
                NotificationEffect::EmitUpdate(event) => {
                    self.listeners.retain(|(_, listener)| {
                        let directive = listener.on_update_event(event.clone());
                        if let Some(validating_listener) = listener.as_validating_listener() {
                            match event {
                                UpdateEvent::Validating => validating_listener.notify_validating(),
                                UpdateEvent::Validated => validating_listener.notify_validated(),
                                UpdateEvent::Painting { .. }
                                | UpdateEvent::Painted { .. }
                                | UpdateEvent::Prepared { .. }
                                | UpdateEvent::Submitted { .. } => {}
                            }
                        }
                        directive == ListenerDirective::Keep
                    });
                }
                NotificationEffect::EmitAncestor(event) => {
                    self.ancestor_listeners.retain(|(_, listener)| {
                        listener.ancestor_changed(*event) == ListenerDirective::Keep
                    });
                }
                NotificationEffect::EmitProperty(event) => {
                    self.property_listeners.retain(|(_, listener)| {
                        listener.property_changed(event) == ListenerDirective::Keep
                    });
                }
                NotificationEffect::EmitAction(event) => {
                    self.action_listeners.retain(|(_, listener)| {
                        listener.action_performed(*event) == ListenerDirective::Keep
                    });
                }
                NotificationEffect::EmitLayout(event) => {
                    self.layout_listeners.retain(|(_, listener)| {
                        listener.layout_changed(*event) == ListenerDirective::Keep
                    });
                }
            }
        }
    }

    fn absorb_graph_effects(&mut self, graph: &mut crate::graph::FigureTree) {
        self.notification_effects
            .extend(graph.drain_notification_effects());
    }

    pub(crate) fn take_animation_effects(
        &mut self,
        graph: &mut crate::graph::FigureTree,
    ) -> Vec<NotificationEffect> {
        self.absorb_graph_effects(graph);
        let effects = self.notification_effects.effects();
        let cursor = self.animation_effect_cursor.min(effects.len());
        let unconsumed = effects[cursor..].to_vec();
        self.animation_effect_cursor = effects.len();
        unconsumed
    }

    /// 统一 flush：收集 FigureTree 和 UpdateManager 两边的 effect，
    /// 在事务边界统一分发到所有注册的 listener。
    pub(crate) fn flush_notifications(&mut self, graph: &mut crate::graph::FigureTree) {
        self.absorb_graph_effects(graph);
        let effects = self.notification_effects.drain();
        self.animation_effect_cursor = 0;
        self.dispatch_effects(&effects, graph, self.publication_epoch);
        self.retain_live_listener_scopes();
    }

    pub(crate) fn flush_notifications_at(
        &mut self,
        graph: &mut crate::graph::FigureTree,
        stable_epoch: u64,
    ) {
        self.publication_epoch = stable_epoch;
        self.flush_notifications(graph);
    }

    pub(crate) fn set_publication_epoch(&mut self, stable_epoch: u64) {
        self.publication_epoch = stable_epoch;
    }

    fn retain_live_listener_scopes(&mut self) {
        let live: std::collections::HashSet<_> = self
            .listeners
            .iter()
            .map(|(id, _)| *id)
            .chain(self.figure_listeners.iter().map(|(id, _)| *id))
            .chain(self.coordinate_listeners.iter().map(|(id, _)| *id))
            .chain(self.ancestor_listeners.iter().map(|(id, _)| *id))
            .chain(self.property_listeners.iter().map(|(id, _)| *id))
            .chain(self.action_listeners.iter().map(|(id, _)| *id))
            .chain(self.layout_listeners.iter().map(|(id, _)| *id))
            .chain(self.observation_listeners.iter().map(|(id, _)| *id))
            .collect();
        self.listener_owners.retain(|id, _| live.contains(id));
    }

    /// 添加脏区域
    ///
    /// 对应 draw2d: UpdateManager.addDirtyRegion()
    ///
    /// # Arguments
    ///
    /// * `figure_id` - 需要重绘的块 ID
    /// * `rect` - node-local 脏区域
    pub fn add_dirty_region(&mut self, figure_id: FigureId, rect: Rectangle) {
        if merge_dirty_region(&mut self.dirty_regions, figure_id, rect) {
            self.update_queued = true;
        }
    }

    pub(crate) fn add_frozen_surface_region(&mut self, rect: Rectangle) {
        if rect.width > 0.0 && rect.height > 0.0 {
            self.frozen_surface_regions.push(rect);
            self.update_queued = true;
        }
    }

    /// 添加失效块
    ///
    /// 对应 draw2d: UpdateManager.addInvalidFigure()
    ///
    /// 失效的块将在下一帧进行布局计算。
    ///
    /// # Arguments
    ///
    /// * `figure_id` - 需要重新布局的块 ID
    pub fn add_invalid_figure(&mut self, figure_id: FigureId) {
        // 检查是否已在队列中
        if self.invalid_figures.contains(&figure_id) {
            return;
        }

        self.invalid_figures.push(figure_id);
        self.update_queued = true;
    }

    /// 检查是否有待处理的布局
    pub fn has_pending_layout(&self) -> bool {
        !self.invalid_figures.is_empty()
    }

    /// 检查是否有待处理的重绘
    pub fn has_pending_repaint(&self) -> bool {
        !self.dirty_regions.is_empty() || !self.frozen_surface_regions.is_empty()
    }

    pub fn last_validation_error(&self) -> Option<&ValidationError> {
        self.last_validation_error.as_ref()
    }

    pub fn take_validation_error(&mut self) -> Option<ValidationError> {
        self.last_validation_error.take()
    }

    pub(crate) fn emit_update_event(&mut self, event: UpdateEvent) {
        self.notification_effects.emit_update(event);
    }

    pub(crate) fn enqueue_notification_effect(&mut self, effect: NotificationEffect) {
        self.notification_effects.extend([effect]);
    }

    /// 检查是否有待处理的更新
    ///
    /// 对应 draw2d: updateQueued flag
    pub fn is_update_queued(&self) -> bool {
        self.update_queued
    }

    /// 计算合并后的脏区域
    ///
    /// 将所有脏区域合并为一个大的区域。
    pub fn compute_damage(&self) -> Rectangle {
        compute_damage_union(
            self.dirty_regions
                .values()
                .chain(self.frozen_surface_regions.iter()),
        )
    }

    pub(crate) fn take_dirty_snapshot(&mut self) -> std::collections::HashMap<FigureId, Rectangle> {
        std::mem::take(&mut self.dirty_regions)
    }

    /// 清空所有待处理的更新
    pub fn clear(&mut self) {
        self.dirty_regions.clear();
        self.frozen_surface_regions.clear();
        self.invalid_figures.clear();
        self.update_queued = false;
        self.updating = false;
        self.last_validation_error = None;
        self.notification_effects.drain();
        self.animation_effect_cursor = 0;
    }

    /// 返回当前积累的更新通知 effect。
    pub(crate) fn notification_effects(&self) -> &[NotificationEffect] {
        self.notification_effects.effects()
    }

    /// 获取失效块数量
    #[allow(dead_code)]
    pub fn invalid_count(&self) -> usize {
        self.invalid_figures.len()
    }

    /// 获取脏区域数量
    #[allow(dead_code)]
    pub fn dirty_count(&self) -> usize {
        self.dirty_regions.len() + self.frozen_surface_regions.len()
    }

    /// 排空并返回所有待验证的块 ID
    ///
    /// 对应 draw2d: performValidation 中对 invalidFigures 的 drain。
    /// FigureTree 使用此方法获取需要验证的块列表。
    pub fn drain_invalid_figures(&mut self) -> Vec<FigureId> {
        self.invalid_figures.drain(..).collect()
    }

    /// 清空脏区域和更新标记
    ///
    /// 对应 draw2d: performUpdate 完成后清空队列。
    /// 由 FigureTree 在 repairDamage 完成后调用。
    pub fn clear_dirty_and_flag(&mut self) {
        self.update_queued = !self.invalid_figures.is_empty()
            || !self.dirty_regions.is_empty()
            || !self.frozen_surface_regions.is_empty();
    }

    fn restore_dirty_snapshot(
        &mut self,
        dirty_snapshot: std::collections::HashMap<FigureId, Rectangle>,
    ) {
        for (figure_id, rect) in dirty_snapshot {
            merge_dirty_region(&mut self.dirty_regions, figure_id, rect);
        }
    }

    fn perform_update_transaction(
        &mut self,
        graph: &mut crate::graph::FigureTree,
        canvas: &mut NdCanvas,
        presentation: Option<&crate::animation::PresentationSnapshot>,
        dirty_snapshot: &mut Option<std::collections::HashMap<FigureId, Rectangle>>,
        frozen_surface_snapshot: &mut Option<Vec<Rectangle>>,
    ) -> Result<(), ValidationError> {
        self.absorb_graph_effects(graph);

        self.perform_validation_phase(graph)?;

        self.update_queued = false;
        *dirty_snapshot = Some(self.take_dirty_snapshot());
        *frozen_surface_snapshot = Some(std::mem::take(&mut self.frozen_surface_regions));
        let snapshot = dirty_snapshot
            .as_ref()
            .expect("dirty snapshot must exist during repair");
        let frozen_surface = frozen_surface_snapshot
            .as_ref()
            .expect("frozen surface snapshot must exist during repair");
        let damage = prepare_damage_set(
            graph,
            canvas,
            snapshot.iter(),
            frozen_surface.iter().copied(),
        );

        if !snapshot.is_empty() || !frozen_surface.is_empty() {
            let reported_damage = damage.unwrap_or_else(|| Rectangle::new(0.0, 0.0, 0.0, 0.0));
            self.notification_effects
                .emit_update(UpdateEvent::Painting {
                    damage: reported_damage,
                });
            if damage.is_some() {
                graph.render_to_with_presentation(canvas, presentation);
            }
            self.notification_effects.emit_update(UpdateEvent::Painted {
                damage: reported_damage,
            });
        }

        self.clear_dirty_and_flag();
        self.flush_notifications(graph);
        Ok(())
    }

    pub(crate) fn perform_validation_phase(
        &mut self,
        graph: &mut crate::graph::FigureTree,
    ) -> Result<(), ValidationError> {
        if !self.has_pending_layout() {
            return Ok(());
        }
        self.last_validation_error = None;
        self.notification_effects
            .emit_update(UpdateEvent::Validating);
        match graph.perform_validation_cycle(self) {
            Ok(()) => {
                self.absorb_graph_effects(graph);
                self.notification_effects
                    .emit_update(UpdateEvent::Validated);
                Ok(())
            }
            Err(error) => {
                self.last_validation_error = Some(error.clone());
                for figure_id in graph.invalid_figure_ids() {
                    self.add_invalid_figure(figure_id);
                }
                self.notification_effects.retain_semantic_effects();
                self.clear_dirty_and_flag();
                Err(error)
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn perform_update(
        &mut self,
        graph: &mut crate::graph::FigureTree,
        canvas: &mut NdCanvas,
    ) {
        self.perform_update_with_presentation(graph, canvas, None);
    }

    pub(crate) fn perform_update_with_presentation(
        &mut self,
        graph: &mut crate::graph::FigureTree,
        canvas: &mut NdCanvas,
        presentation: Option<&crate::animation::PresentationSnapshot>,
    ) {
        if self.updating {
            return;
        }

        self.updating = true;
        self.last_validation_error = None;
        let mut dirty_snapshot = None;
        let mut frozen_surface_snapshot = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.perform_update_transaction(
                graph,
                canvas,
                presentation,
                &mut dirty_snapshot,
                &mut frozen_surface_snapshot,
            )
        }));
        self.updating = false;

        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                self.last_validation_error = Some(error);
                for figure_id in graph.invalid_figure_ids() {
                    self.add_invalid_figure(figure_id);
                }
                self.notification_effects.retain_semantic_effects();
                self.clear_dirty_and_flag();
            }
            Err(payload) => {
                if let Some(snapshot) = dirty_snapshot {
                    self.restore_dirty_snapshot(snapshot);
                }
                if let Some(snapshot) = frozen_surface_snapshot {
                    self.frozen_surface_regions.extend(snapshot);
                }
                for figure_id in graph.invalid_figure_ids() {
                    self.add_invalid_figure(figure_id);
                }
                self.notification_effects.retain_semantic_effects();
                self.clear_dirty_and_flag();
                std::panic::resume_unwind(payload);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn perform_validation(&mut self, graph: &mut crate::graph::FigureTree) {
        self.last_validation_error = graph.perform_validation_cycle(self).err();
    }

    pub fn is_updating(&self) -> bool {
        self.updating
    }
}

fn remove_listener<T: ?Sized>(listeners: &mut Vec<(ListenerId, Box<T>)>, id: ListenerId) -> bool {
    let old_len = listeners.len();
    listeners.retain(|(listener_id, _)| *listener_id != id);
    listeners.len() != old_len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::geometry::Dimension;
    use crate::{
        AncestorEvent, AncestorListener, CoordinateListener, Figure, FigureEvent, FigureId,
        FigureListener, FigureMeasurement, FigureTree, LayoutError, LayoutEvent, LayoutListener,
        LayoutManager, LayoutOutput, LayoutSnapshot, MeasureConstraints, PropertyChangeEvent,
        PropertyChangeListener, RectangleFigure, StackLayout, XYConstraint, XYLayout,
    };
    use slotmap::KeyData;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn create_test_key(data: u64) -> FigureId {
        FigureId::from(KeyData::from_ffi(data))
    }

    struct PanicOnceLayout {
        did_panic: AtomicBool,
    }

    struct PanicPaintFigure;

    impl Figure for PanicPaintFigure {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }

        fn name(&self) -> &'static str {
            "PanicPaintFigure"
        }

        fn paint_figure(&self, _gc: &mut NdCanvas) {
            panic!("intentional paint panic");
        }
    }

    impl LayoutManager for PanicOnceLayout {
        fn preferred_measurement(
            &self,
            _container: FigureId,
            _constraints: MeasureConstraints,
            _snapshot: &LayoutSnapshot<'_>,
        ) -> FigureMeasurement {
            FigureMeasurement::default()
        }

        fn minimum_size(
            &self,
            container: FigureId,
            constraints: MeasureConstraints,
            snapshot: &LayoutSnapshot<'_>,
        ) -> Dimension {
            self.preferred_measurement(container, constraints, snapshot)
                .size()
        }

        fn layout(
            &mut self,
            _container: FigureId,
            _snapshot: &LayoutSnapshot<'_>,
            _out: &mut LayoutOutput,
        ) -> Result<(), LayoutError> {
            if !self.did_panic.swap(true, Ordering::SeqCst) {
                panic!("intentional layout panic");
            }
            Ok(())
        }
    }

    #[test]
    fn test_dirty_region_tracking() {
        let mut manager = UpdateManager::new();

        let rect = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        manager.add_dirty_region(create_test_key(1), rect);

        assert!(manager.is_update_queued());
        assert!(manager.has_pending_repaint());
        assert_eq!(manager.dirty_count(), 1);
    }

    #[test]
    fn test_dirty_region_merge() {
        let mut manager = UpdateManager::new();

        let rect1 = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        let rect2 = Rectangle::new(50.0, 50.0, 100.0, 100.0);

        let key = create_test_key(1);
        manager.add_dirty_region(key, rect1);
        manager.add_dirty_region(key, rect2);

        // 应该合并为一个区域
        assert_eq!(manager.dirty_count(), 1);

        let damage = manager.compute_damage();
        assert_eq!(damage.x, 0.0);
        assert_eq!(damage.y, 0.0);
        assert_eq!(damage.width, 150.0);
        assert_eq!(damage.height, 150.0);
    }

    #[test]
    fn test_invalid_block_queue() {
        let mut manager = UpdateManager::new();

        let key = create_test_key(1);
        manager.add_invalid_figure(key);

        assert!(manager.has_pending_layout());
        assert_eq!(manager.invalid_count(), 1);
    }

    #[test]
    fn test_invalid_block_dedup() {
        let mut manager = UpdateManager::new();

        let key = create_test_key(1);
        manager.add_invalid_figure(key);
        manager.add_invalid_figure(key); // 重复添加

        // 应该去重
        assert_eq!(manager.invalid_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut manager = UpdateManager::new();

        let key = create_test_key(1);
        manager.add_dirty_region(key, Rectangle::new(0.0, 0.0, 100.0, 100.0));
        manager.add_invalid_figure(key);

        manager.clear();

        assert!(!manager.is_update_queued());
        assert!(!manager.has_pending_layout());
        assert!(!manager.has_pending_repaint());
    }

    #[test]
    fn test_invalid_region() {
        let mut manager = UpdateManager::new();

        // 无效区域应该被忽略
        let rect = Rectangle::new(0.0, 0.0, 0.0, 100.0);
        manager.add_dirty_region(create_test_key(1), rect);

        assert!(!manager.has_pending_repaint());
    }

    #[test]
    fn test_drain_invalid_blocks() {
        let mut manager = UpdateManager::new();

        manager.add_invalid_figure(create_test_key(1));
        manager.add_invalid_figure(create_test_key(2));

        let drained = manager.drain_invalid_figures();
        assert_eq!(drained.len(), 2);
        assert!(!manager.has_pending_layout());
    }

    #[test]
    fn test_take_dirty_snapshot_freezes_current_cycle() {
        let mut manager = UpdateManager::new();
        let key1 = create_test_key(1);
        let key2 = create_test_key(2);
        manager.add_dirty_region(key1, Rectangle::new(0.0, 0.0, 10.0, 10.0));

        let snapshot = manager.take_dirty_snapshot();
        manager.add_dirty_region(key2, Rectangle::new(20.0, 20.0, 5.0, 5.0));

        assert_eq!(snapshot.len(), 1);
        assert_eq!(
            snapshot.get(&key1),
            Some(&Rectangle::new(0.0, 0.0, 10.0, 10.0))
        );
        assert!(!snapshot.contains_key(&key2));
        assert_eq!(manager.dirty_count(), 1);
        assert_eq!(
            manager.dirty_regions.get(&key2),
            Some(&Rectangle::new(20.0, 20.0, 5.0, 5.0))
        );
    }

    #[test]
    fn test_clear_dirty_and_flag_preserves_next_cycle_work() {
        let mut manager = UpdateManager::new();
        manager.add_dirty_region(create_test_key(1), Rectangle::new(0.0, 0.0, 10.0, 10.0));
        let _snapshot = manager.take_dirty_snapshot();
        manager.add_dirty_region(create_test_key(2), Rectangle::new(5.0, 5.0, 5.0, 5.0));

        manager.clear_dirty_and_flag();

        assert!(manager.is_update_queued());
        assert!(manager.has_pending_repaint());
    }

    #[test]
    fn test_perform_update_writes_damage_set_to_canvas() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            400.0,
            300.0,
            Color::rgba(0.1, 0.1, 0.1, 1.0),
        )));
        manager.add_dirty_region(root_id, Rectangle::new(10.0, 20.0, 30.0, 40.0));

        let mut canvas = NdCanvas::new();
        manager.perform_update(&mut graph, &mut canvas);

        assert_eq!(
            canvas.damage().union(),
            Some(Rectangle::new(10.0, 20.0, 30.0, 40.0))
        );
        assert_eq!(
            canvas.damage().regions(),
            &[Rectangle::new(10.0, 20.0, 30.0, 40.0)]
        );
    }

    #[test]
    fn test_perform_update_records_update_phase_effects() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            400.0,
            300.0,
            Color::rgba(0.1, 0.1, 0.1, 1.0),
        )));
        manager.add_dirty_region(root_id, Rectangle::new(10.0, 20.0, 30.0, 40.0));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        {
            struct CaptureUpdate {
                effects: std::sync::Arc<std::sync::Mutex<Vec<NotificationEffect>>>,
            }
            impl UpdateListener for CaptureUpdate {
                fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
                    self.effects
                        .lock()
                        .unwrap()
                        .push(NotificationEffect::EmitUpdate(event));
                    ListenerDirective::Keep
                }
                fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
                    ListenerDirective::Keep
                }
                fn on_notify(&self, _figure_id: FigureId) -> ListenerDirective {
                    ListenerDirective::Keep
                }
            }
            manager.add_listener(Box::new(CaptureUpdate {
                effects: effects.clone(),
            }));
        }

        let mut canvas = NdCanvas::new();
        manager.perform_update(&mut graph, &mut canvas);

        let captured = effects.lock().unwrap().clone();
        assert_eq!(
            captured,
            vec![
                NotificationEffect::EmitUpdate(UpdateEvent::Painting {
                    damage: Rectangle::new(10.0, 20.0, 30.0, 40.0),
                }),
                NotificationEffect::EmitUpdate(UpdateEvent::Painted {
                    damage: Rectangle::new(10.0, 20.0, 30.0, 40.0),
                }),
            ]
        );
    }

    #[test]
    fn test_update_notifications_report_root_domain_damage() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
        let coordinate_root = graph.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(100.0, 50.0, 200.0, 150.0)),
        );
        let child = graph.add_child_to(
            coordinate_root,
            Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)),
        );
        manager.add_dirty_region(child, Rectangle::new(0.0, 0.0, 30.0, 40.0));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CapturePainting {
            effects: std::sync::Arc<std::sync::Mutex<Vec<UpdateEvent>>>,
        }
        impl UpdateListener for CapturePainting {
            fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
                self.effects.lock().unwrap().push(event);
                ListenerDirective::Keep
            }
            fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
                ListenerDirective::Keep
            }
            fn on_notify(&self, _figure_id: FigureId) -> ListenerDirective {
                ListenerDirective::Keep
            }
        }
        manager.add_listener(Box::new(CapturePainting {
            effects: effects.clone(),
        }));

        let mut canvas = NdCanvas::new();
        manager.perform_update(&mut graph, &mut canvas);

        let expected = Rectangle::new(110.0, 70.0, 30.0, 40.0);
        assert_eq!(canvas.damage().union(), Some(expected));
        assert!(
            effects
                .lock()
                .unwrap()
                .contains(&UpdateEvent::Painting { damage: expected })
        );
    }

    #[test]
    fn test_clipped_damage_notifies_without_rendering() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        manager.add_dirty_region(root_id, Rectangle::new(200.0, 200.0, 10.0, 10.0));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureUpdate {
            effects: std::sync::Arc<std::sync::Mutex<Vec<UpdateEvent>>>,
        }
        impl UpdateListener for CaptureUpdate {
            fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
                self.effects.lock().unwrap().push(event);
                ListenerDirective::Keep
            }
            fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
                ListenerDirective::Keep
            }
            fn on_notify(&self, _figure_id: FigureId) -> ListenerDirective {
                ListenerDirective::Keep
            }
        }
        manager.add_listener(Box::new(CaptureUpdate {
            effects: effects.clone(),
        }));

        let mut canvas = NdCanvas::new();
        manager.perform_update(&mut graph, &mut canvas);

        let empty_damage = Rectangle::new(0.0, 0.0, 0.0, 0.0);
        assert!(canvas.damage().is_empty());
        assert!(canvas.commands().is_empty());
        assert_eq!(
            *effects.lock().unwrap(),
            vec![
                UpdateEvent::Painting {
                    damage: empty_damage,
                },
                UpdateEvent::Painted {
                    damage: empty_damage,
                },
            ]
        );
        assert!(!manager.is_update_queued());
    }

    #[test]
    fn test_update_without_dirty_regions_skips_rendering() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureUpdate {
            effects: std::sync::Arc<std::sync::Mutex<Vec<UpdateEvent>>>,
        }
        impl UpdateListener for CaptureUpdate {
            fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
                self.effects.lock().unwrap().push(event);
                ListenerDirective::Keep
            }
            fn on_figure_event(&self, _event: FigureEvent) -> ListenerDirective {
                ListenerDirective::Keep
            }
            fn on_notify(&self, _figure_id: FigureId) -> ListenerDirective {
                ListenerDirective::Keep
            }
        }
        manager.add_listener(Box::new(CaptureUpdate {
            effects: effects.clone(),
        }));

        let mut canvas = NdCanvas::new();
        manager.perform_update(&mut graph, &mut canvas);

        assert!(canvas.damage().is_empty());
        assert!(canvas.commands().is_empty());
        assert!(effects.lock().unwrap().is_empty());
    }

    #[test]
    fn test_direct_invalid_queue_entry_invalidates_and_validates_graph_node() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        graph.revalidate(root_id);
        assert!(graph.is_valid(root_id));

        manager.add_invalid_figure(root_id);
        manager.perform_update(&mut graph, &mut NdCanvas::new());

        assert!(graph.is_valid(root_id));
        assert!(!manager.is_update_queued());
    }

    #[test]
    fn test_update_panic_restores_manager_state_and_requeues_invalid_graph_nodes() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        graph.revalidate(root_id);
        graph.replace_layout_manager(
            root_id,
            Some(Box::new(PanicOnceLayout {
                did_panic: AtomicBool::new(false),
            })),
        );
        manager.add_invalid_figure(root_id);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            manager.perform_update(&mut graph, &mut NdCanvas::new());
        }));

        assert!(result.is_err());
        assert!(!manager.is_updating());
        assert!(manager.has_pending_layout());
        assert!(manager.is_update_queued());

        manager.perform_update(&mut graph, &mut NdCanvas::new());
        assert!(graph.is_valid(root_id));
        assert!(!manager.is_update_queued());
    }

    #[test]
    fn test_update_panic_restores_frozen_surface_damage() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        graph.set_contents(Box::new(PanicPaintFigure));
        let frozen = Rectangle::new(10.0, 20.0, 30.0, 40.0);
        manager.add_frozen_surface_region(frozen);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            manager.perform_update(&mut graph, &mut NdCanvas::new());
        }));

        assert!(result.is_err());
        assert_eq!(manager.frozen_surface_regions, vec![frozen]);
        assert!(manager.has_pending_repaint());
        assert!(manager.is_update_queued());
    }

    #[test]
    fn test_validation_figure_effects_preserve_causal_order() {
        let mut manager = UpdateManager::new();
        let mut graph = FigureTree::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
        let child_id = graph.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)),
        );
        graph.replace_layout_manager(root_id, Some(Box::new(XYLayout::new())));
        graph.set_boxed_constraint(
            child_id,
            Box::new(XYConstraint::at_size(30.0, 40.0, 50.0, 60.0)),
        );
        graph.revalidate(root_id);
        graph.drain_notification_effects();
        graph.set_boxed_constraint(
            child_id,
            Box::new(XYConstraint::at_size(60.0, 70.0, 50.0, 60.0)),
        );
        graph.mark_invalid(&mut manager, child_id);

        let effects = Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureAll {
            effects: Arc<std::sync::Mutex<Vec<NotificationEffect>>>,
        }
        impl UpdateListener for CaptureAll {
            fn on_update_event(&self, event: UpdateEvent) -> ListenerDirective {
                self.effects
                    .lock()
                    .unwrap()
                    .push(NotificationEffect::EmitUpdate(event));
                ListenerDirective::Keep
            }
            fn on_figure_event(&self, event: FigureEvent) -> ListenerDirective {
                self.effects
                    .lock()
                    .unwrap()
                    .push(NotificationEffect::EmitFigure(event));
                ListenerDirective::Keep
            }
            fn on_notify(&self, figure_id: FigureId) -> ListenerDirective {
                self.effects
                    .lock()
                    .unwrap()
                    .push(NotificationEffect::Notify { figure_id });
                ListenerDirective::Keep
            }
        }
        manager.add_listener(Box::new(CaptureAll {
            effects: effects.clone(),
        }));

        manager.perform_update(&mut graph, &mut NdCanvas::new());

        let captured = effects.lock().unwrap();
        let validating = captured
            .iter()
            .position(|effect| *effect == NotificationEffect::EmitUpdate(UpdateEvent::Validating))
            .expect("validating event");
        let moved = captured
            .iter()
            .position(|effect| {
                matches!(
                    effect,
                    NotificationEffect::EmitFigure(FigureEvent::FigureMoved {
                        figure_id,
                        ..
                    }) if *figure_id == child_id
                )
            })
            .expect("figure moved event");
        let validated = captured
            .iter()
            .position(|effect| *effect == NotificationEffect::EmitUpdate(UpdateEvent::Validated))
            .expect("validated event");

        assert!(validating < moved);
        assert!(moved < validated);
    }

    #[derive(Default)]
    struct TypedListenerCounts {
        figure: usize,
        coordinate: usize,
        ancestor: usize,
        property: usize,
        layout: usize,
    }

    struct TypedListener {
        counts: Arc<std::sync::Mutex<TypedListenerCounts>>,
    }

    impl FigureListener for TypedListener {
        fn figure_moved(&self, _event: FigureEvent) -> ListenerDirective {
            self.counts.lock().unwrap().figure += 1;
            ListenerDirective::Keep
        }
    }

    impl CoordinateListener for TypedListener {
        fn coordinate_system_changed(&self, _event: FigureEvent) -> ListenerDirective {
            self.counts.lock().unwrap().coordinate += 1;
            ListenerDirective::Keep
        }
    }

    impl AncestorListener for TypedListener {
        fn ancestor_changed(&self, _event: AncestorEvent) -> ListenerDirective {
            self.counts.lock().unwrap().ancestor += 1;
            ListenerDirective::Keep
        }
    }

    impl PropertyChangeListener for TypedListener {
        fn property_changed(&self, _event: &PropertyChangeEvent) -> ListenerDirective {
            self.counts.lock().unwrap().property += 1;
            ListenerDirective::Keep
        }
    }

    impl LayoutListener for TypedListener {
        fn layout_changed(&self, _event: LayoutEvent) -> ListenerDirective {
            self.counts.lock().unwrap().layout += 1;
            ListenerDirective::Keep
        }
    }

    #[test]
    fn test_typed_listeners_dispatch_and_remove_independently() {
        let mut manager = UpdateManager::new();
        let counts = Arc::new(std::sync::Mutex::new(TypedListenerCounts::default()));
        let figure_id = manager.add_figure_listener(Box::new(TypedListener {
            counts: counts.clone(),
        }));
        let coordinate_id = manager.add_coordinate_listener(Box::new(TypedListener {
            counts: counts.clone(),
        }));
        let ancestor_id = manager.add_ancestor_listener(Box::new(TypedListener {
            counts: counts.clone(),
        }));
        let property_id = manager.add_property_listener(Box::new(TypedListener {
            counts: counts.clone(),
        }));
        let layout_id = manager.add_layout_listener(Box::new(TypedListener {
            counts: counts.clone(),
        }));

        let mut graph = FigureTree::new();
        let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child =
            graph.add_child_to(root, Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0)));
        graph.replace_layout_manager(root, Some(Box::new(StackLayout::new())));
        graph.set_visible(child, false);
        graph.set_visible(child, true);
        graph.prim_translate(root, 5.0, 5.0);
        graph.mark_invalid(&mut manager, root);
        manager.perform_update(&mut graph, &mut NdCanvas::new());

        {
            let counts = counts.lock().unwrap();
            assert!(counts.figure > 0);
            assert!(counts.coordinate > 0);
            assert!(counts.ancestor > 0);
            assert!(counts.property > 0);
            assert!(counts.layout > 0);
        }

        assert!(manager.remove_listener(figure_id));
        assert!(manager.remove_listener(coordinate_id));
        assert!(manager.remove_listener(ancestor_id));
        assert!(manager.remove_listener(property_id));
        assert!(manager.remove_listener(layout_id));
        assert!(!manager.remove_listener(layout_id));

        let before = {
            let counts = counts.lock().unwrap();
            (
                counts.figure,
                counts.coordinate,
                counts.ancestor,
                counts.property,
                counts.layout,
            )
        };
        graph.set_visible(child, false);
        graph.prim_translate(child, 1.0, 1.0);
        graph.mark_invalid(&mut manager, root);
        manager.perform_update(&mut graph, &mut NdCanvas::new());
        let after = counts.lock().unwrap();
        assert_eq!(
            before,
            (
                after.figure,
                after.coordinate,
                after.ancestor,
                after.property,
                after.layout,
            )
        );
    }
}
