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

use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use crate::graph::BlockId;
use crate::runtime::update::listener::{
    AncestorListener, CoordinateListener, FigureListener, LayoutListener, ListenerId,
    NotificationEffect, NotificationQueue, PropertyChangeListener, UpdateEvent, UpdateListener,
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
/// - 纯数据管理：具体的验证和渲染由 FigureGraph 通过 trait 方法执行
///
/// # 与 draw2d 的差异
///
/// draw2d 的 DeferredUpdateManager 直接持有 root Figure 引用并直接调用其方法。
/// 本实现将数据管理（UM）和业务逻辑（FigureGraph）分离，
/// 通过 `UpdateManagerSource` trait 定义回调接口，保持解耦。
pub struct SceneUpdateManager {
    /// 脏区域映射：block_id -> 脏区域
    pub(crate) dirty_regions: std::collections::HashMap<BlockId, Rectangle>,
    /// 失效块队列
    pub(crate) invalid_blocks: Vec<BlockId>,
    /// 是否有更新待处理
    pub(crate) update_queued: bool,
    pub(crate) updating: bool,
    notification_effects: NotificationQueue,
    listeners: Vec<(ListenerId, Box<dyn UpdateListener>)>,
    figure_listeners: Vec<(ListenerId, Box<dyn FigureListener>)>,
    coordinate_listeners: Vec<(ListenerId, Box<dyn CoordinateListener>)>,
    ancestor_listeners: Vec<(ListenerId, Box<dyn AncestorListener>)>,
    property_listeners: Vec<(ListenerId, Box<dyn PropertyChangeListener>)>,
    layout_listeners: Vec<(ListenerId, Box<dyn LayoutListener>)>,
    next_listener_id: u64,
}

impl Default for SceneUpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneUpdateManager {
    /// 创建新的场景更新管理器
    pub fn new() -> Self {
        Self {
            dirty_regions: std::collections::HashMap::new(),
            invalid_blocks: Vec::new(),
            update_queued: false,
            updating: false,
            notification_effects: NotificationQueue::new(),
            listeners: Vec::new(),
            figure_listeners: Vec::new(),
            coordinate_listeners: Vec::new(),
            ancestor_listeners: Vec::new(),
            property_listeners: Vec::new(),
            layout_listeners: Vec::new(),
            next_listener_id: 1,
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

    pub fn add_layout_listener(&mut self, listener: Box<dyn LayoutListener>) -> ListenerId {
        let id = self.allocate_listener_id();
        self.layout_listeners.push((id, listener));
        id
    }

    pub fn remove_listener(&mut self, id: ListenerId) -> bool {
        remove_listener(&mut self.listeners, id)
            || remove_listener(&mut self.figure_listeners, id)
            || remove_listener(&mut self.coordinate_listeners, id)
            || remove_listener(&mut self.ancestor_listeners, id)
            || remove_listener(&mut self.property_listeners, id)
            || remove_listener(&mut self.layout_listeners, id)
    }

    fn allocate_listener_id(&mut self) -> ListenerId {
        let id = ListenerId::new(self.next_listener_id);
        self.next_listener_id = self
            .next_listener_id
            .checked_add(1)
            .expect("listener id space exhausted");
        id
    }

    /// 向所有监听器分发 effect 队列中的事件
    fn dispatch_effects(&self, effects: &[NotificationEffect]) {
        for effect in effects {
            match effect {
                NotificationEffect::Notify { block_id } => {
                    for (_, listener) in &self.listeners {
                        listener.on_notify(*block_id);
                    }
                }
                NotificationEffect::EmitFigure(event) => {
                    for (_, listener) in &self.listeners {
                        listener.on_figure_event(*event);
                    }
                    match event {
                        crate::FigureEvent::FigureMoved { .. } => {
                            for (_, listener) in &self.figure_listeners {
                                listener.figure_moved(*event);
                            }
                        }
                        crate::FigureEvent::CoordinateSystemChanged { .. } => {
                            for (_, listener) in &self.coordinate_listeners {
                                listener.coordinate_system_changed(*event);
                            }
                        }
                    }
                }
                NotificationEffect::EmitUpdate(event) => {
                    for (_, listener) in &self.listeners {
                        listener.on_update_event(event.clone());
                        if let Some(validating_listener) = listener.as_validating_listener() {
                            match event {
                                UpdateEvent::Validating => validating_listener.notify_validating(),
                                UpdateEvent::Validated => validating_listener.notify_validated(),
                                UpdateEvent::Painting { .. } | UpdateEvent::Painted { .. } => {}
                            }
                        }
                    }
                }
                NotificationEffect::EmitAncestor(event) => {
                    for (_, listener) in &self.ancestor_listeners {
                        listener.ancestor_changed(*event);
                    }
                }
                NotificationEffect::EmitProperty(event) => {
                    for (_, listener) in &self.property_listeners {
                        listener.property_changed(event);
                    }
                }
                NotificationEffect::EmitLayout(event) => {
                    for (_, listener) in &self.layout_listeners {
                        listener.layout_changed(*event);
                    }
                }
            }
        }
    }

    fn absorb_graph_effects(&mut self, graph: &mut crate::graph::FigureGraph) {
        self.notification_effects
            .extend(graph.drain_notification_effects());
    }

    /// 统一 flush：收集 FigureGraph 和 SceneUpdateManager 两边的 effect，
    /// 在事务边界统一分发到所有注册的 listener。
    pub fn flush_notifications(&mut self, graph: &mut crate::graph::FigureGraph) {
        self.absorb_graph_effects(graph);
        let effects = self.notification_effects.drain();
        self.dispatch_effects(&effects);
    }

    /// 添加脏区域
    ///
    /// 对应 draw2d: UpdateManager.addDirtyRegion()
    ///
    /// # Arguments
    ///
    /// * `block_id` - 需要重绘的块 ID
    /// * `rect` - 脏区域（与该 block 的 bounds 同域）
    pub fn add_dirty_region(&mut self, block_id: BlockId, rect: Rectangle) {
        if merge_dirty_region(&mut self.dirty_regions, block_id, rect) {
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
    /// * `block_id` - 需要重新布局的块 ID
    pub fn add_invalid_figure(&mut self, block_id: BlockId) {
        // 检查是否已在队列中
        if self.invalid_blocks.contains(&block_id) {
            return;
        }

        self.invalid_blocks.push(block_id);
        self.update_queued = true;
    }

    /// 检查是否有待处理的布局
    pub fn has_pending_layout(&self) -> bool {
        !self.invalid_blocks.is_empty()
    }

    /// 检查是否有待处理的重绘
    pub fn has_pending_repaint(&self) -> bool {
        !self.dirty_regions.is_empty()
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
        compute_damage_union(self.dirty_regions.values())
    }

    pub(crate) fn take_dirty_snapshot(&mut self) -> std::collections::HashMap<BlockId, Rectangle> {
        std::mem::take(&mut self.dirty_regions)
    }

    /// 清空所有待处理的更新
    pub fn clear(&mut self) {
        self.dirty_regions.clear();
        self.invalid_blocks.clear();
        self.update_queued = false;
        self.updating = false;
        self.notification_effects.drain();
    }

    /// 返回当前积累的更新通知 effect。
    pub fn notification_effects(&self) -> &[NotificationEffect] {
        self.notification_effects.effects()
    }

    /// 排空更新通知 effect。
    pub fn drain_notification_effects(&mut self) -> Vec<NotificationEffect> {
        self.notification_effects.drain()
    }

    /// 获取失效块数量
    #[allow(dead_code)]
    pub fn invalid_count(&self) -> usize {
        self.invalid_blocks.len()
    }

    /// 获取脏区域数量
    #[allow(dead_code)]
    pub fn dirty_count(&self) -> usize {
        self.dirty_regions.len()
    }

    /// 排空并返回所有待验证的块 ID
    ///
    /// 对应 draw2d: performValidation 中对 invalidFigures 的 drain。
    /// FigureGraph 使用此方法获取需要验证的块列表。
    pub fn drain_invalid_blocks(&mut self) -> Vec<BlockId> {
        self.invalid_blocks.drain(..).collect()
    }

    /// 清空脏区域和更新标记
    ///
    /// 对应 draw2d: performUpdate 完成后清空队列。
    /// 由 FigureGraph 在 repairDamage 完成后调用。
    pub fn clear_dirty_and_flag(&mut self) {
        self.update_queued = !self.invalid_blocks.is_empty() || !self.dirty_regions.is_empty();
    }

    fn restore_dirty_snapshot(
        &mut self,
        dirty_snapshot: std::collections::HashMap<BlockId, Rectangle>,
    ) {
        for (block_id, rect) in dirty_snapshot {
            merge_dirty_region(&mut self.dirty_regions, block_id, rect);
        }
    }

    fn perform_update_transaction(
        &mut self,
        graph: &mut crate::graph::FigureGraph,
        canvas: &mut NdCanvas,
        dirty_snapshot: &mut Option<std::collections::HashMap<BlockId, Rectangle>>,
    ) {
        self.absorb_graph_effects(graph);

        if self.has_pending_layout() {
            self.notification_effects
                .emit_update(UpdateEvent::Validating);
            graph.perform_validation_cycle(self);
            self.absorb_graph_effects(graph);
            self.notification_effects
                .emit_update(UpdateEvent::Validated);
        }

        self.update_queued = false;
        *dirty_snapshot = Some(self.take_dirty_snapshot());
        let snapshot = dirty_snapshot
            .as_ref()
            .expect("dirty snapshot must exist during repair");
        let damage = prepare_damage_set(graph, canvas, snapshot.iter());

        if !snapshot.is_empty() {
            let reported_damage = damage.unwrap_or_else(|| Rectangle::new(0.0, 0.0, 0.0, 0.0));
            self.notification_effects
                .emit_update(UpdateEvent::Painting {
                    damage: reported_damage,
                });
            if damage.is_some() {
                graph.render_to(canvas);
            }
            self.notification_effects.emit_update(UpdateEvent::Painted {
                damage: reported_damage,
            });
        }

        self.clear_dirty_and_flag();
        self.flush_notifications(graph);
    }
}

fn remove_listener<T: ?Sized>(listeners: &mut Vec<(ListenerId, Box<T>)>, id: ListenerId) -> bool {
    let old_len = listeners.len();
    listeners.retain(|(listener_id, _)| *listener_id != id);
    listeners.len() != old_len
}

impl crate::runtime::update::UpdateManager for SceneUpdateManager {
    fn add_dirty_region(&mut self, block_id: BlockId, rect: Rectangle) {
        SceneUpdateManager::add_dirty_region(self, block_id, rect);
    }

    fn add_invalid_figure(&mut self, block_id: BlockId) {
        SceneUpdateManager::add_invalid_figure(self, block_id);
    }

    fn enqueue_notification_effect(&mut self, effect: NotificationEffect) {
        self.notification_effects.extend([effect]);
    }

    fn drain_invalid_blocks(&mut self) -> Vec<BlockId> {
        SceneUpdateManager::drain_invalid_blocks(self)
    }

    fn perform_update(&mut self, graph: &mut crate::graph::FigureGraph, canvas: &mut NdCanvas) {
        if self.updating {
            return;
        }

        self.updating = true;
        let mut dirty_snapshot = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.perform_update_transaction(graph, canvas, &mut dirty_snapshot);
        }));
        self.updating = false;

        if let Err(payload) = result {
            if let Some(snapshot) = dirty_snapshot {
                self.restore_dirty_snapshot(snapshot);
            }
            for block_id in graph.invalid_block_ids() {
                self.add_invalid_figure(block_id);
            }
            self.notification_effects.retain_semantic_effects();
            self.clear_dirty_and_flag();
            std::panic::resume_unwind(payload);
        }
    }

    fn perform_validation(&mut self, graph: &mut crate::graph::FigureGraph) {
        graph.perform_validation_cycle(self);
    }

    fn is_updating(&self) -> bool {
        self.updating
    }

    fn is_update_queued(&self) -> bool {
        self.update_queued
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AncestorEvent, AncestorListener, CoordinateListener, FigureEvent, FigureGraph,
        FigureListener, LayoutEvent, LayoutListener, LayoutManager, PropertyChangeEvent,
        PropertyChangeListener, RectangleFigure, StackLayout, XYConstraint, XYLayout,
        layout::LayoutContext, scene::BlockId, update::UpdateManager,
    };
    use novadraw_core::Color;
    use slotmap::KeyData;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn create_test_key(data: u64) -> BlockId {
        BlockId::from(KeyData::from_ffi(data))
    }

    struct PanicOnceLayout {
        did_panic: AtomicBool,
    }

    impl LayoutManager for PanicOnceLayout {
        fn get_preferred_size(
            &self,
            _container: BlockId,
            _w_hint: f64,
            _h_hint: f64,
            _ctx: &dyn LayoutContext,
        ) -> (f64, f64) {
            (0.0, 0.0)
        }

        fn get_minimum_size(
            &self,
            container: BlockId,
            w_hint: f64,
            h_hint: f64,
            ctx: &dyn LayoutContext,
        ) -> (f64, f64) {
            self.get_preferred_size(container, w_hint, h_hint, ctx)
        }

        fn layout(&self, _container: BlockId, _ctx: &mut dyn LayoutContext) {
            if !self.did_panic.swap(true, Ordering::SeqCst) {
                panic!("intentional layout panic");
            }
        }
    }

    #[test]
    fn test_dirty_region_tracking() {
        let mut manager = SceneUpdateManager::new();

        let rect = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        manager.add_dirty_region(create_test_key(1), rect);

        assert!(manager.is_update_queued());
        assert!(manager.has_pending_repaint());
        assert_eq!(manager.dirty_count(), 1);
    }

    #[test]
    fn test_dirty_region_merge() {
        let mut manager = SceneUpdateManager::new();

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
        let mut manager = SceneUpdateManager::new();

        let key = create_test_key(1);
        manager.add_invalid_figure(key);

        assert!(manager.has_pending_layout());
        assert_eq!(manager.invalid_count(), 1);
    }

    #[test]
    fn test_invalid_block_dedup() {
        let mut manager = SceneUpdateManager::new();

        let key = create_test_key(1);
        manager.add_invalid_figure(key);
        manager.add_invalid_figure(key); // 重复添加

        // 应该去重
        assert_eq!(manager.invalid_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut manager = SceneUpdateManager::new();

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
        let mut manager = SceneUpdateManager::new();

        // 无效区域应该被忽略
        let rect = Rectangle::new(0.0, 0.0, 0.0, 100.0);
        manager.add_dirty_region(create_test_key(1), rect);

        assert!(!manager.has_pending_repaint());
    }

    #[test]
    fn test_drain_invalid_blocks() {
        let mut manager = SceneUpdateManager::new();

        manager.add_invalid_figure(create_test_key(1));
        manager.add_invalid_figure(create_test_key(2));

        let drained = manager.drain_invalid_blocks();
        assert_eq!(drained.len(), 2);
        assert!(!manager.has_pending_layout());
    }

    #[test]
    fn test_take_dirty_snapshot_freezes_current_cycle() {
        let mut manager = SceneUpdateManager::new();
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
        let mut manager = SceneUpdateManager::new();
        manager.add_dirty_region(create_test_key(1), Rectangle::new(0.0, 0.0, 10.0, 10.0));
        let _snapshot = manager.take_dirty_snapshot();
        manager.add_dirty_region(create_test_key(2), Rectangle::new(5.0, 5.0, 5.0, 5.0));

        manager.clear_dirty_and_flag();

        assert!(manager.is_update_queued());
        assert!(manager.has_pending_repaint());
    }

    #[test]
    fn test_perform_update_writes_damage_set_to_canvas() {
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
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
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
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
                fn on_update_event(&self, event: UpdateEvent) {
                    self.effects
                        .lock()
                        .unwrap()
                        .push(NotificationEffect::EmitUpdate(event));
                }
                fn on_figure_event(&self, _event: FigureEvent) {}
                fn on_notify(&self, _block_id: BlockId) {}
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
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
        let coordinate_root = graph.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(100.0, 50.0, 200.0, 150.0).with_local_coordinates(true)),
        );
        let child = graph.add_child_to(
            coordinate_root,
            Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 40.0)),
        );
        manager.add_dirty_region(child, Rectangle::new(10.0, 20.0, 30.0, 40.0));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CapturePainting {
            effects: std::sync::Arc<std::sync::Mutex<Vec<UpdateEvent>>>,
        }
        impl UpdateListener for CapturePainting {
            fn on_update_event(&self, event: UpdateEvent) {
                self.effects.lock().unwrap().push(event);
            }
            fn on_figure_event(&self, _event: FigureEvent) {}
            fn on_notify(&self, _block_id: BlockId) {}
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
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        manager.add_dirty_region(root_id, Rectangle::new(200.0, 200.0, 10.0, 10.0));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureUpdate {
            effects: std::sync::Arc<std::sync::Mutex<Vec<UpdateEvent>>>,
        }
        impl UpdateListener for CaptureUpdate {
            fn on_update_event(&self, event: UpdateEvent) {
                self.effects.lock().unwrap().push(event);
            }
            fn on_figure_event(&self, _event: FigureEvent) {}
            fn on_notify(&self, _block_id: BlockId) {}
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
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
        graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));

        let effects = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureUpdate {
            effects: std::sync::Arc<std::sync::Mutex<Vec<UpdateEvent>>>,
        }
        impl UpdateListener for CaptureUpdate {
            fn on_update_event(&self, event: UpdateEvent) {
                self.effects.lock().unwrap().push(event);
            }
            fn on_figure_event(&self, _event: FigureEvent) {}
            fn on_notify(&self, _block_id: BlockId) {}
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
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
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
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        graph.revalidate(root_id);
        graph.set_block_layout_manager(
            root_id,
            Arc::new(PanicOnceLayout {
                did_panic: AtomicBool::new(false),
            }),
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
    fn test_validation_figure_effects_preserve_causal_order() {
        let mut manager = SceneUpdateManager::new();
        let mut graph = FigureGraph::new();
        let root_id = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)));
        let child_id = graph.add_child_to(
            root_id,
            Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 20.0)),
        );
        graph.set_block_layout_manager(root_id, Arc::new(XYLayout::new()));
        graph.set_constraint(child_id, XYConstraint::at_size(30.0, 40.0, 50.0, 60.0));
        graph.revalidate(root_id);
        graph.drain_notification_effects();
        graph.set_constraint(child_id, XYConstraint::at_size(60.0, 70.0, 50.0, 60.0));
        graph.mark_invalid(&mut manager, child_id);

        let effects = Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureAll {
            effects: Arc<std::sync::Mutex<Vec<NotificationEffect>>>,
        }
        impl UpdateListener for CaptureAll {
            fn on_update_event(&self, event: UpdateEvent) {
                self.effects
                    .lock()
                    .unwrap()
                    .push(NotificationEffect::EmitUpdate(event));
            }
            fn on_figure_event(&self, event: FigureEvent) {
                self.effects
                    .lock()
                    .unwrap()
                    .push(NotificationEffect::EmitFigure(event));
            }
            fn on_notify(&self, block_id: BlockId) {
                self.effects
                    .lock()
                    .unwrap()
                    .push(NotificationEffect::Notify { block_id });
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
                        block_id,
                        ..
                    }) if *block_id == child_id
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
        fn figure_moved(&self, _event: FigureEvent) {
            self.counts.lock().unwrap().figure += 1;
        }
    }

    impl CoordinateListener for TypedListener {
        fn coordinate_system_changed(&self, _event: FigureEvent) {
            self.counts.lock().unwrap().coordinate += 1;
        }
    }

    impl AncestorListener for TypedListener {
        fn ancestor_changed(&self, _event: AncestorEvent) {
            self.counts.lock().unwrap().ancestor += 1;
        }
    }

    impl PropertyChangeListener for TypedListener {
        fn property_changed(&self, _event: &PropertyChangeEvent) {
            self.counts.lock().unwrap().property += 1;
        }
    }

    impl LayoutListener for TypedListener {
        fn layout_changed(&self, _event: LayoutEvent) {
            self.counts.lock().unwrap().layout += 1;
        }
    }

    #[test]
    fn test_typed_listeners_dispatch_and_remove_independently() {
        let mut manager = SceneUpdateManager::new();
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

        let mut graph = FigureGraph::new();
        let root = graph.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)));
        let child = graph.add_child_to(
            root,
            Box::new(RectangleFigure::new(10.0, 10.0, 20.0, 20.0).with_local_coordinates(true)),
        );
        graph.set_block_layout_manager(root, Arc::new(StackLayout::new()));
        graph.set_visible(child, false);
        graph.set_visible(child, true);
        graph.prim_translate(child, 5.0, 5.0);
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
