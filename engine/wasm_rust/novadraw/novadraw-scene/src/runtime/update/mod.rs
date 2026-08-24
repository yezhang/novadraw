//! Update Manager - 更新管理
//!
//! 管理场景图的更新流程，包括：
//! - 脏区域（dirty region）跟踪：记录需要重绘的区域
//! - 失效块（invalid block）队列：记录需要重新布局的块
//! - 两阶段更新：先布局（validation），再重绘（repaint）
//!
//! 参考 Eclipse Draw2D 的 DeferredUpdateManager 设计。
//!
//! # 更新流程
//!
//! ```text
//! repaint() ──────► add_dirty_region() ──► 脏区域队列
//!                                                          │
//! revalidate() ──► add_invalid_figure() ──► 失效块队列   │
//!                                                          ▼
//!                                             perform_update()
//!                                                  │
//!                    ┌──────────────────────────────┼──────────────────────────────┐
//!                    ▼                              ▼                              ▼
//!              Phase 1: Layout            Phase 2: Union Dirty Regions    Phase 3: Repaint
//!              (布局失效的块)              (合并所有脏区域)                (重绘脏区域)
//! ```
//!
//! # 架构设计
//!
//! [`SceneUpdateManager`] 是纯数据管理器，负责跟踪脏区域和失效块队列。
//! [`FigureGraph`] 是 orchestrator，通过调用 SceneUpdateManager 的数据方法
//! 编排两阶段更新流程。
//! [`repair`] 模块负责 DamageSet 写入与 repair phase 的脏区处理逻辑。
//!
//! 对应 draw2d 的 DeferredUpdateManager（持有 root + GraphicsSource + orchestrator），
//! 本实现将数据管理（SceneUpdateManager）和场景编排（FigureGraph）分离，
//! 通过 trait [`UpdateManagerSource`] 定义 FigureGraph 的回调接口，
//! 支持未来替换不同的更新策略实现。

mod deferred;
mod listener;
mod repair;

pub use deferred::SceneUpdateManager;
pub use listener::{
    AncestorEvent, AncestorEventKind, AncestorListener, CoordinateListener, FigureEvent,
    FigureListener, LayoutEvent, LayoutEventKind, LayoutListener, ListenerId, NotificationEffect,
    NotificationQueue, PropertyChangeEvent, PropertyChangeListener, PropertyValue, UpdateEvent,
    UpdateListener, ValidatingListener,
};

pub trait UpdateManager: Send + Sync {
    fn add_dirty_region(
        &mut self,
        block_id: crate::graph::BlockId,
        rect: novadraw_geometry::Rectangle,
    );
    fn add_invalid_figure(&mut self, block_id: crate::graph::BlockId);
    fn drain_invalid_blocks(&mut self) -> Vec<crate::graph::BlockId>;
    fn perform_update(
        &mut self,
        graph: &mut crate::graph::FigureGraph,
        canvas: &mut novadraw_render::NdCanvas,
    );
    fn perform_validation(&mut self, graph: &mut crate::graph::FigureGraph);
    fn is_update_queued(&self) -> bool;
    fn is_updating(&self) -> bool;
}

/// Update Manager Source - 更新管理器数据源
///
/// 对应 draw2d: DeferredUpdateManager 持有 root Figure 的方式。
///
/// 定义 FigureGraph 作为 UpdateManager 数据源时需要实现的接口。
/// 当 SceneUpdateManager 需要执行验证和渲染时，通过此 trait 回调 FigureGraph。
///
/// # 实现说明
///
/// 当前 validation 的图级语义由 FigureGraph 执行，
/// SceneUpdateManager 只负责保存队列并触发 phase 边界。
///
/// # 设计要点
///
/// - draw2d 的 DeferredUpdateManager 直接持有 root Figure 引用并调用其方法
/// - 本实现通过 trait 定义回调接口，保持 SceneUpdateManager 与 FigureGraph 解耦
pub trait UpdateManagerSource: Send + Sync {
    /// 执行单个块的布局验证
    ///
    /// 对应 draw2d: Figure.validate()
    ///
    /// # Arguments
    ///
    /// * `block_id` - 需要验证的块 ID
    fn perform_validation(&mut self, block_id: crate::graph::BlockId);

    /// 使用脏区域裁剪渲染场景
    ///
    /// 对应 draw2d: DeferredUpdateManager.repairDamage() 中的 paint(graphics)
    ///
    /// # Arguments
    ///
    /// * `clip` - 脏区域裁剪矩形
    fn render_damage(&mut self, clip: novadraw_geometry::Rectangle) -> novadraw_render::NdCanvas;
}
