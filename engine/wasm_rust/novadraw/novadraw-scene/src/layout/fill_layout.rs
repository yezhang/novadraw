//! Fill 布局器
//!
//! 参考 draw2d: FlowLayout 或 FillLayout
//! 第一个子元素填充容器，其他子元素保持原位。

use tracing::debug;

use super::LayoutContext;
use super::LayoutManager;
use crate::graph::BlockId;

/// Fill 布局器
///
/// 第一个子元素填充容器（减去 insets），其他子元素保持原位。
#[derive(Debug, Clone)]
pub struct FillLayout;

impl FillLayout {
    /// 创建新的 FillLayout
    pub fn new() -> Self {
        Self
    }
}

impl Default for FillLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutManager for FillLayout {
    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        ctx.get_children(container)
            .first()
            .map(|(child, _)| ctx.get_preferred_size(*child, w_hint, h_hint))
            .unwrap_or((0.0, 0.0))
    }

    fn get_minimum_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        ctx.get_children(container)
            .first()
            .map(|(child, _)| ctx.get_minimum_size(*child, w_hint, h_hint))
            .unwrap_or((0.0, 0.0))
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let children = ctx.get_children(container);
        if children.is_empty() {
            return;
        }

        debug!(
            "FillLayout: container={:?}, children count: {}",
            container,
            children.len()
        );

        // 获取第一个子元素
        if let Some((first_child_id, _)) = children.first() {
            // FillLayout：第一个子元素填充容器的 client area
            let bounds = ctx.get_container_bounds(container);
            debug!("FillLayout: first child bounds={:?}", bounds);
            ctx.set_child_bounds(*first_child_id, bounds);
        }
    }
}
