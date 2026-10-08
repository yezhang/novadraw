//! Fill 布局器
//!
//! 参考 draw2d: FlowLayout 或 FillLayout
//! 第一个子元素填充容器，其他子元素保持原位。

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::geometry::Dimension;
use crate::{FigureMeasurement, MeasureConstraints, graph::FigureId};

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
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        snapshot
            .children(container)
            .first()
            .map(|(child, _)| snapshot.preferred_measurement(*child, constraints))
            .unwrap_or_default()
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        snapshot
            .children(container)
            .first()
            .map(|(child, _)| snapshot.minimum_size(*child, constraints))
            .unwrap_or_default()
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let children = snapshot.children(container);
        if children.is_empty() {
            return Ok(());
        }

        // 获取第一个子元素
        if let Some((first_child_id, _)) = children.first() {
            // FillLayout：第一个子元素填充容器的 client area
            let bounds = snapshot.container_bounds(container);
            out.set_child_bounds(*first_child_id, bounds);
        }
        Ok(())
    }
}
