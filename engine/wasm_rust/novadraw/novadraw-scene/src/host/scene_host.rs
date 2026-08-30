//! Platform host and legacy SceneHost contracts.
//!
//! 定义渲染入口与平台环境交互的接口。只负责渲染触发和视口管理，
//! 不持有 FigureGraph、UpdateManager 等核心对象。
//!
//! 对应 draw2d: LightweightSystem 的**渲染入口职责**（paint() 方法）
//!
//! # LightweightSystem 职责分散说明
//!
//! LightweightSystem 持有 Canvas + UpdateManager + EventDispatcher + RootFigure，
//! 这些职责在 Novadraw 中分散到多个组件：
//!
//! | LightweightSystem 持有 | Novadraw 对应 |
//! |----------------------|---------------|
//! | root Figure | FigureGraph.root |
//! | UpdateManager | NovadrawSystem.update_manager |
//! | EventDispatcher | EventDispatcher trait |
//! | Canvas / paint entry | SceneHost + RenderBackend |
//!
//! SceneHost 只对应 LightweightSystem 的：
//! - 渲染入口：execute_update()
//! - 视口大小：viewport_size()
//! - 更新请求：request_update()
//!
//! # 设计理念
//!
//! draw2d 的 LightweightSystem 通过 SWT 的 `Display.asyncExec` 实现延迟批处理。
//! 本实现将调度策略抽象为 trait，不同平台提供不同实现：
//!
//! - **WinitSceneHost**: `request_update()` → `window.request_redraw()`，
//!   利用 winit 的 redraw 请求实现 request-driven 帧合并
//! - **WebSceneHost** (未来): `requestAnimationFrame` 调度
//!
//! # 更新流程
//!
//! ```text
//! FigureGraph.mark_invalid() / repaint()
//!         │
//!         ▼ (组合根检测到 UpdateManager 队列从空变为非空)
//! SceneHost.request_update()
//!         │
//!         ▼ (平台调度)
//! WinitSceneHost:  window.request_redraw()
//!         │
//!         ▼ (RedrawRequested 事件)
//! SceneHost.execute_update()
//!         │
//!         └─► update_manager.perform_update(scene, canvas)
//!                 │
//!                 ▼
//!         renderer.render(commands)
//! ```
//!
//! # 职责边界
//!
//! - **SceneHost**: 渲染入口协调。不持有任何核心对象（FigureGraph、UpdateManager 等）。
//! - **FigureGraph**: 块树管理 + 布局计算。平台无关。
//! - **EventDispatcher**: 平台无关事件状态机；新 Runtime 的交互状态独立于 FigureTree。
//! - **平台事件循环**: 负责把 winit redraw/input 事件转交给组合根。

use novadraw_render::{NdCanvas, RenderBackend, SurfaceInfo};

use crate::{FigureGraph, UpdateManager};

/// Minimal platform boundary used by the runtime.
///
/// Input adaptation and surface ownership stay in platform crates. The runtime
/// only needs normalized surface information and an idempotent redraw request.
pub trait PlatformHost {
    fn request_redraw(&self);
    fn surface_info(&self) -> SurfaceInfo;
}

/// 场景图主机环境 trait
///
/// 定义场景图与平台环境交互的接口。
///
/// # 实现说明
///
/// 典型实现（WinitSceneHost）：
/// 1. `request_update()` 设置 host 侧 `update_queued = true`
/// 2. 平台事件循环收到 `RedrawRequested` → 调用 `execute_update()`
/// 3. `execute_update()` 执行两阶段更新后，用 UpdateManager 队列状态同步 host 标记
///
/// 不同平台可提供不同的调度策略（如 requestAnimationFrame、节流等）。
pub trait SceneHost {
    /// 请求在下一次渲染帧执行更新
    ///
    /// 多次调用应合并为一次（由具体实现保证，如 winit 的 request_redraw 已是幂等）。
    fn request_update(&self);

    /// 检查是否有待执行的更新
    ///
    /// 对应 draw2d: DeferredUpdateManager.updateQueued 标志。
    fn is_update_queued(&self) -> bool;

    /// 执行一轮完整的两阶段更新（布局验证 + 脏区域重绘）
    ///
    /// 对应 draw2d: DeferredUpdateManager.performUpdate()。
    ///
    fn execute_update(
        &self,
        scene: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        renderer: &mut impl RenderBackend,
    ) -> NdCanvas;

    /// 获取视口（窗口）尺寸
    ///
    /// # Returns
    ///
    /// `(width, height)` 单位为逻辑像素
    fn viewport_size(&self) -> (f64, f64);
}
