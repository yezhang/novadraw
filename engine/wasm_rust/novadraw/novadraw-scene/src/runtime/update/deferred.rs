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
    NotificationEffect, NotificationQueue, UpdateEvent, UpdateListener,
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
    listeners: Vec<Box<dyn UpdateListener>>,
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
        }
    }

    /// 注册更新监听器
    pub fn add_listener(&mut self, listener: Box<dyn UpdateListener>) {
        self.listeners.push(listener);
    }

    /// 向所有监听器分发 effect 队列中的事件
    fn dispatch_effects(&self, effects: &[NotificationEffect]) {
        for effect in effects {
            match effect {
                NotificationEffect::Notify { block_id } => {
                    for listener in &self.listeners {
                        listener.on_notify(*block_id);
                    }
                }
                NotificationEffect::EmitFigure(event) => {
                    for listener in &self.listeners {
                        listener.on_figure_event(*event);
                    }
                }
                NotificationEffect::EmitUpdate(event) => {
                    for listener in &self.listeners {
                        listener.on_update_event(event.clone());
                    }
                }
            }
        }
    }

    /// 统一 flush：收集 FigureGraph 和 SceneUpdateManager 两边的 effect，
    /// 在事务边界统一分发到所有注册的 listener。
    pub fn flush_notifications(&mut self, graph: &mut crate::graph::FigureGraph) {
        let um_effects = self.notification_effects.drain();
        self.dispatch_effects(&um_effects);

        let graph_effects = graph.drain_notification_effects();
        self.dispatch_effects(&graph_effects);
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
}

impl crate::runtime::update::UpdateManager for SceneUpdateManager {
    fn add_dirty_region(&mut self, block_id: BlockId, rect: Rectangle) {
        SceneUpdateManager::add_dirty_region(self, block_id, rect);
    }

    fn add_invalid_figure(&mut self, block_id: BlockId) {
        SceneUpdateManager::add_invalid_figure(self, block_id);
    }

    fn drain_invalid_blocks(&mut self) -> Vec<BlockId> {
        SceneUpdateManager::drain_invalid_blocks(self)
    }

    fn perform_update(&mut self, graph: &mut crate::graph::FigureGraph, canvas: &mut NdCanvas) {
        if self.updating {
            return;
        }

        self.updating = true;
        self.notification_effects
            .emit_update(UpdateEvent::Validating);
        self.perform_validation(graph);
        self.notification_effects
            .emit_update(UpdateEvent::Validated);
        self.update_queued = false;
        let dirty_snapshot = self.take_dirty_snapshot();
        let damage = prepare_damage_set(graph, canvas, dirty_snapshot.iter());

        if !dirty_snapshot.is_empty() {
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
        self.updating = false;
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
    use crate::{FigureEvent, FigureGraph, RectangleFigure, scene::BlockId, update::UpdateManager};
    use novadraw_core::Color;
    use slotmap::KeyData;

    fn create_test_key(data: u64) -> BlockId {
        BlockId::from(KeyData::from_ffi(data))
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
                NotificationEffect::EmitUpdate(UpdateEvent::Validating),
                NotificationEffect::EmitUpdate(UpdateEvent::Validated),
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
                UpdateEvent::Validating,
                UpdateEvent::Validated,
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
        assert_eq!(
            *effects.lock().unwrap(),
            vec![UpdateEvent::Validating, UpdateEvent::Validated]
        );
    }
}
