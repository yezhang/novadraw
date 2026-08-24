//! Winit 平台 SceneHost 实现
//!
//! 对应 draw2d: LightweightSystem 的平台 paint 调度入口职责。
//!
//! # 调度策略
//!
//! 利用 winit 的 `request_redraw` 实现 request-driven 帧合并：
//! - `request_update()` → `window.request_redraw()`（幂等，系统自动去重）
//! - `RedrawRequested` 事件 → `execute_update()` 执行两阶段更新

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use novadraw::{
    FigureGraph, NdCanvas, RenderBackend, RenderOutcome, SceneHost, UpdateManager,
    backend::vello::WinitWindowProxy, traits::WindowProxy,
};

/// Winit 平台的 SceneHost 实现
///
/// 持有 winit 窗口引用和 redraw 挂起标记，协调平台 redraw 入口。
///
/// 不持有 FigureGraph / UpdateManager；核心对象由组合根在调用 `execute_update()` 时传入。
pub struct WinitSceneHost {
    window: Arc<WinitWindowProxy>,
    /// 是否有待执行的更新
    update_queued: AtomicBool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UpdateExecution {
    None,
    Full,
    Incremental,
}

fn select_update_execution(host_requested: bool, manager_queued: bool) -> UpdateExecution {
    if manager_queued {
        UpdateExecution::Incremental
    } else if host_requested {
        UpdateExecution::Full
    } else {
        UpdateExecution::None
    }
}

impl WinitSceneHost {
    /// 创建新的 WinitSceneHost
    pub fn new(window: Arc<WinitWindowProxy>) -> Self {
        Self {
            window,
            update_queued: AtomicBool::new(false),
        }
    }
}

impl SceneHost for WinitSceneHost {
    fn request_update(&self) {
        if !self.update_queued.swap(true, Ordering::AcqRel) {
            self.window.request_redraw();
        }
    }

    fn is_update_queued(&self) -> bool {
        self.update_queued.load(Ordering::Acquire)
    }

    fn execute_update(
        &self,
        scene: &mut FigureGraph,
        update_manager: &mut dyn UpdateManager,
        renderer: &mut impl RenderBackend,
    ) -> NdCanvas {
        let host_requested = self.update_queued.swap(false, Ordering::AcqRel);
        let manager_queued = update_manager.is_update_queued();
        let canvas = match select_update_execution(host_requested, manager_queued) {
            UpdateExecution::None => return NdCanvas::new(),
            UpdateExecution::Full => scene.render(),
            UpdateExecution::Incremental => scene.perform_update(update_manager),
        };
        if !canvas.damage().is_empty()
            && renderer.render(&canvas.to_submission()) == RenderOutcome::Retry
        {
            self.request_update();
        }

        if update_manager.is_update_queued() {
            self.request_update();
        }
        canvas
    }

    fn viewport_size(&self) -> (f64, f64) {
        (self.window.width() as f64, self.window.height() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_only_request_forces_full_render() {
        assert_eq!(select_update_execution(true, false), UpdateExecution::Full);
    }

    #[test]
    fn manager_work_uses_incremental_update() {
        assert_eq!(
            select_update_execution(false, true),
            UpdateExecution::Incremental
        );
        assert_eq!(
            select_update_execution(true, true),
            UpdateExecution::Incremental
        );
    }

    #[test]
    fn no_request_is_a_noop() {
        assert_eq!(select_update_execution(false, false), UpdateExecution::None);
    }
}
