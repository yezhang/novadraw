//! 布局管理
//!
//! 提供 LayoutManager 接口和常用布局器实现，参考 Eclipse Draw2D 设计。

mod border_layout;
mod fill_layout;
mod flow_layout;
mod grid_layout;
mod stack_layout;
mod toolbar_layout;
mod xy_layout;

pub use border_layout::{BorderConstraint, BorderLayout, BorderRegion};
pub use fill_layout::FillLayout;
pub use flow_layout::{FlowDirection, FlowLayout};
pub use grid_layout::{GridAlignment, GridConstraint, GridLayout};
pub use stack_layout::StackLayout;
pub use toolbar_layout::{MinorAlignment, ToolbarLayout, ToolbarOrientation};
pub use xy_layout::{XYConstraint, XYLayout};

use crate::graph::BlockId;
use novadraw_geometry::Rectangle;
use std::any::Any;

/// 容器施加给直接子节点的布局约束。
///
/// 约束由父 FigureBlock 持有；具体 LayoutManager 通过 downcast 读取自己支持的类型。
pub trait LayoutConstraint: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Send + Sync> LayoutConstraint for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// 布局上下文 trait
///
/// 提供布局器所需的场景图查询接口。
pub trait LayoutContext: Send + Sync {
    /// 获取子元素列表
    ///
    /// 返回 (child_id, current_bounds) 列表
    fn get_children(&self, parent_id: BlockId) -> Vec<(BlockId, Rectangle)>;

    /// 获取子元素的布局约束
    fn get_constraint(&self, child_id: BlockId) -> Option<&dyn LayoutConstraint>;

    /// 获取块的首选尺寸
    fn get_preferred_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> (f64, f64);

    /// 获取块的最小尺寸。
    fn get_minimum_size(&self, block_id: BlockId, w_hint: f64, h_hint: f64) -> (f64, f64) {
        self.get_preferred_size(block_id, w_hint, h_hint)
    }

    /// 获取块的最大尺寸。
    fn get_maximum_size(&self, _block_id: BlockId) -> (f64, f64) {
        (f64::INFINITY, f64::INFINITY)
    }

    /// 设置子元素的边界
    ///
    /// `bounds` 必须处于该子元素所属的坐标域中。
    fn set_child_bounds(&mut self, child_id: BlockId, bounds: Rectangle);

    /// Sets layout-controlled child visibility.
    fn set_child_visible(&mut self, _child_id: BlockId, _visible: bool) {}

    /// 获取容器 client area 在子节点坐标域中的矩形。
    fn get_container_bounds(&self, container_id: BlockId) -> Rectangle;
}

/// 布局管理器 trait
///
/// 参考 draw2d: LayoutManager
/// 用于计算和设置子元素的位置。
pub trait LayoutManager: Send + Sync {
    /// 获取首选大小
    ///
    /// 对应 draw2d: getPreferredSize(IFigure, int, int)
    /// wHint, hHint 为建议的宽高，-1 表示无限制
    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64);

    /// 获取最小大小
    ///
    /// 对应 draw2d: getMinimumSize(IFigure, int, int)
    fn get_minimum_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64);

    /// 执行布局
    ///
    /// 对应 draw2d: layout(IFigure)
    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext);
}
