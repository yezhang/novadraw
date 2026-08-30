use novadraw_render::{NdCanvas, RenderBackend};

pub trait NovadrawSystem {
    fn render(&mut self, renderer: &mut impl RenderBackend) -> NdCanvas;
    fn viewport_size(&self) -> (f64, f64);

    /// 请求平台宿主在下一次 redraw 执行更新。
    ///
    /// 该方法属于组合根/SceneHost 调度边界；`UpdateManager` 不应直接调用它。
    fn request_update(&self);
}
