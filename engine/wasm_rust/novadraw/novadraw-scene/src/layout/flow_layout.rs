//! Flow 布局器
//!
//! 参考 draw2d: FlowLayout
//! 按顺序排列子元素，自动换行。

use tracing::debug;

use super::LayoutContext;
use super::LayoutManager;
use crate::graph::BlockId;
use novadraw_geometry::Rectangle;

/// Flow 布局方向
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FlowDirection {
    /// 水平流动（从左到右，然后换行）
    #[default]
    Horizontal,
    /// 垂直流动（从上到下，然后换列）
    Vertical,
}

/// Flow 布局器
///
/// 按顺序排列子元素，自动换行。
///
/// # 使用方式
///
/// 子元素按添加顺序排列，自动换行到下一行/列。
#[derive(Debug, Clone)]
pub struct FlowLayout {
    /// 布局方向
    direction: FlowDirection,
    /// 主轴间距（元素之间的间距）
    spacing: f64,
    /// 行间距
    row_spacing: f64,
}

impl FlowLayout {
    /// 创建新的 FlowLayout（水平方向）
    pub fn new() -> Self {
        Self {
            direction: FlowDirection::Horizontal,
            spacing: 10.0,
            row_spacing: 10.0,
        }
    }

    /// 创建指定方向的 FlowLayout
    pub fn with_direction(direction: FlowDirection) -> Self {
        Self {
            direction,
            spacing: 10.0,
            row_spacing: 10.0,
        }
    }

    /// 设置间距
    pub fn with_spacing(mut self, spacing: f64) -> Self {
        self.spacing = spacing;
        self
    }

    /// 设置行间距
    pub fn with_row_spacing(mut self, row_spacing: f64) -> Self {
        self.row_spacing = row_spacing;
        self
    }

    fn measure(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
        minimum: bool,
    ) -> (f64, f64) {
        let sizes = ctx
            .get_children(container)
            .into_iter()
            .map(|(child, _)| {
                if minimum {
                    ctx.get_minimum_size(child, w_hint, h_hint)
                } else {
                    ctx.get_preferred_size(child, w_hint, h_hint)
                }
            })
            .collect::<Vec<_>>();
        match self.direction {
            FlowDirection::Horizontal => {
                measure_wrapped(&sizes, w_hint, self.spacing, self.row_spacing)
            }
            FlowDirection::Vertical => {
                let transposed = sizes
                    .into_iter()
                    .map(|(width, height)| (height, width))
                    .collect::<Vec<_>>();
                let measured = measure_wrapped(&transposed, h_hint, self.spacing, self.row_spacing);
                (measured.1, measured.0)
            }
        }
    }

    /// 布局计算（内部方法）
    fn perform_layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        let children = ctx.get_children(container);
        if children.is_empty() {
            return;
        }

        debug!(
            "FlowLayout: container={:?}, children count: {}",
            container,
            children.len()
        );

        let container_bounds = ctx.get_container_bounds(container);
        let cx = container_bounds.x;
        let cy = container_bounds.y;
        let cw = container_bounds.width;
        let ch = container_bounds.height;

        debug!(
            "FlowLayout: container bounds=({}, {}, {}, {})",
            cx, cy, cw, ch
        );

        match self.direction {
            FlowDirection::Horizontal => {
                self.layout_horizontal(cx, cy, cw, ch, &children, ctx);
            }
            FlowDirection::Vertical => {
                self.layout_vertical(cx, cy, cw, ch, &children, ctx);
            }
        }
    }

    fn layout_horizontal(
        &self,
        cx: f64,
        cy: f64,
        cw: f64,
        _ch: f64,
        children: &[(BlockId, Rectangle)],
        ctx: &mut dyn LayoutContext,
    ) {
        let mut x: f64 = cx;
        let mut y: f64 = cy;
        let mut row_height: f64 = 0.0;

        for (child_id, _) in children {
            let (child_w, child_h) = ctx.get_preferred_size(*child_id, cw, -1.0);

            // 检查是否需要换行
            if x + child_w > cx + cw && x > cx {
                // 换行
                y += row_height + self.row_spacing;
                x = cx;
                row_height = 0.0;
            }

            // 设置子元素位置
            let new_bounds = Rectangle::new(x, y, child_w, child_h);
            ctx.set_child_bounds(*child_id, new_bounds);

            // 更新位置和行高
            x += child_w + self.spacing;
            row_height = row_height.max(child_h);
        }
    }

    fn layout_vertical(
        &self,
        cx: f64,
        cy: f64,
        _cw: f64,
        ch: f64,
        children: &[(BlockId, Rectangle)],
        ctx: &mut dyn LayoutContext,
    ) {
        let mut x: f64 = cx;
        let mut y: f64 = cy;
        let mut col_width: f64 = 0.0;

        for (child_id, _) in children {
            let (child_w, child_h) = ctx.get_preferred_size(*child_id, -1.0, ch);

            // 检查是否需要换列
            if y + child_h > cy + ch && y > cy {
                // 换列
                x += col_width + self.row_spacing;
                y = cy;
                col_width = 0.0;
            }

            // 设置子元素位置
            let new_bounds = Rectangle::new(x, y, child_w, child_h);
            ctx.set_child_bounds(*child_id, new_bounds);

            // 更新位置和列宽
            y += child_h + self.spacing;
            col_width = col_width.max(child_w);
        }
    }
}

impl Default for FlowLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutManager for FlowLayout {
    fn get_preferred_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.measure(container, w_hint, h_hint, ctx, false)
    }

    fn get_minimum_size(
        &self,
        container: BlockId,
        w_hint: f64,
        h_hint: f64,
        ctx: &dyn LayoutContext,
    ) -> (f64, f64) {
        self.measure(container, w_hint, h_hint, ctx, true)
    }

    fn layout(&self, container: BlockId, ctx: &mut dyn LayoutContext) {
        self.perform_layout(container, ctx);
    }
}

fn measure_wrapped(
    sizes: &[(f64, f64)],
    main_hint: f64,
    spacing: f64,
    line_spacing: f64,
) -> (f64, f64) {
    let mut line_main = 0.0_f64;
    let mut line_minor = 0.0_f64;
    let mut total_main = 0.0_f64;
    let mut total_minor = 0.0_f64;
    let mut line_items = 0_usize;

    for &(main, minor) in sizes {
        let required = if line_items == 0 {
            main
        } else {
            line_main + spacing + main
        };
        if main_hint >= 0.0 && required > main_hint && line_items > 0 {
            total_main = total_main.max(line_main);
            total_minor += line_minor + line_spacing;
            line_main = main;
            line_minor = minor;
            line_items = 1;
        } else {
            line_main = required;
            line_minor = line_minor.max(minor);
            line_items += 1;
        }
    }

    total_main = total_main.max(line_main);
    total_minor += line_minor;
    (total_main, total_minor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_layout_creation() {
        let layout = FlowLayout::new();
        let (w, h) = layout.get_preferred_size(
            BlockId::from(slotmap::KeyData::from_ffi(0)),
            800.0,
            600.0,
            &MockLayoutContext::new(),
        );
        assert_eq!((w, h), (0.0, 0.0));
    }

    #[test]
    fn test_flow_layout_with_direction() {
        let layout = FlowLayout::with_direction(FlowDirection::Vertical);
        assert_eq!(layout.direction, FlowDirection::Vertical);
    }

    #[test]
    fn test_flow_layout_with_spacing() {
        let layout = FlowLayout::new().with_spacing(20.0).with_row_spacing(15.0);
        // 通过 get_preferred_size 间接验证
        let _ = layout.get_preferred_size(
            BlockId::from(slotmap::KeyData::from_ffi(0)),
            800.0,
            600.0,
            &MockLayoutContext::new(),
        );
    }
}

/// Mock LayoutContext for testing
#[cfg(test)]
struct MockLayoutContext {
    children: Vec<(BlockId, Rectangle)>,
    container_bounds: Rectangle,
}

#[cfg(test)]
impl MockLayoutContext {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            container_bounds: Rectangle::new(0.0, 0.0, 800.0, 600.0),
        }
    }
}

#[cfg(test)]
impl super::LayoutContext for MockLayoutContext {
    fn get_children(&self, _parent_id: BlockId) -> Vec<(BlockId, Rectangle)> {
        self.children.clone()
    }

    fn get_constraint(&self, _child_id: BlockId) -> Option<&dyn super::LayoutConstraint> {
        None
    }

    fn get_preferred_size(&self, _block_id: BlockId, _w_hint: f64, _h_hint: f64) -> (f64, f64) {
        (100.0, 100.0)
    }

    fn set_child_bounds(&mut self, _child_id: BlockId, _bounds: Rectangle) {}

    fn get_container_bounds(&self, _container_id: BlockId) -> Rectangle {
        self.container_bounds
    }
}
