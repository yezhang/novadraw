//! Flow 布局器
//!
//! 参考 draw2d: FlowLayout
//! 按顺序排列子元素，自动换行。

use tracing::debug;

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::{FigureMeasurement, MeasureConstraints, graph::FigureId};
use novadraw_geometry::{Dimension, Rectangle};

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
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
        minimum: bool,
    ) -> Dimension {
        let sizes = snapshot
            .children(container)
            .into_iter()
            .map(|(child, _)| {
                if minimum {
                    snapshot.minimum_size(child, constraints)
                } else {
                    snapshot.preferred_measurement(child, constraints).size()
                }
            })
            .collect::<Vec<_>>();
        match self.direction {
            FlowDirection::Horizontal => measure_wrapped(
                &sizes,
                constraints.max_width(),
                self.spacing,
                self.row_spacing,
            ),
            FlowDirection::Vertical => {
                let transposed = sizes
                    .into_iter()
                    .map(|size| Dimension::new(size.height, size.width))
                    .collect::<Vec<_>>();
                let measured = measure_wrapped(
                    &transposed,
                    constraints.max_height(),
                    self.spacing,
                    self.row_spacing,
                );
                Dimension::new(measured.height, measured.width)
            }
        }
    }

    /// 布局计算（内部方法）
    fn perform_layout(
        &self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) {
        let children = snapshot.children(container);
        if children.is_empty() {
            return;
        }

        debug!(
            "FlowLayout: container={:?}, children count: {}",
            container,
            children.len()
        );

        let container_bounds = snapshot.container_bounds(container);
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
                self.layout_horizontal(container_bounds, &children, snapshot, out);
            }
            FlowDirection::Vertical => {
                self.layout_vertical(container_bounds, &children, snapshot, out);
            }
        }
    }

    fn layout_horizontal(
        &self,
        area: Rectangle,
        children: &[(FigureId, Rectangle)],
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) {
        let mut x: f64 = area.x;
        let mut y: f64 = area.y;
        let mut row_height: f64 = 0.0;

        for (child_id, _) in children {
            let measured = snapshot.preferred_measurement(
                *child_id,
                MeasureConstraints::width(area.width)
                    .expect("container width is valid layout geometry"),
            );
            let (child_w, child_h) = (measured.width, measured.height);

            // 检查是否需要换行
            if x + child_w > area.x + area.width && x > area.x {
                // 换行
                y += row_height + self.row_spacing;
                x = area.x;
                row_height = 0.0;
            }

            // 设置子元素位置
            let new_bounds = Rectangle::new(x, y, child_w, child_h);
            out.set_child_bounds(*child_id, new_bounds);

            // 更新位置和行高
            x += child_w + self.spacing;
            row_height = row_height.max(child_h);
        }
    }

    fn layout_vertical(
        &self,
        area: Rectangle,
        children: &[(FigureId, Rectangle)],
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) {
        let mut x: f64 = area.x;
        let mut y: f64 = area.y;
        let mut col_width: f64 = 0.0;

        for (child_id, _) in children {
            let measured = snapshot.preferred_measurement(
                *child_id,
                MeasureConstraints::height(area.height)
                    .expect("container height is valid layout geometry"),
            );
            let (child_w, child_h) = (measured.width, measured.height);

            // 检查是否需要换列
            if y + child_h > area.y + area.height && y > area.y {
                // 换列
                x += col_width + self.row_spacing;
                y = area.y;
                col_width = 0.0;
            }

            // 设置子元素位置
            let new_bounds = Rectangle::new(x, y, child_w, child_h);
            out.set_child_bounds(*child_id, new_bounds);

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
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement {
        let size = self.measure(container, constraints, snapshot, false);
        FigureMeasurement::new(size.width, size.height, None)
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension {
        self.measure(container, constraints, snapshot, true)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.perform_layout(container, snapshot, out);
        Ok(())
    }
}

fn measure_wrapped(
    sizes: &[Dimension],
    max_main: Option<f64>,
    spacing: f64,
    line_spacing: f64,
) -> Dimension {
    let mut line_main = 0.0_f64;
    let mut line_minor = 0.0_f64;
    let mut total_main = 0.0_f64;
    let mut total_minor = 0.0_f64;
    let mut line_items = 0_usize;

    for size in sizes {
        let (main, minor) = (size.width, size.height);
        let required = if line_items == 0 {
            main
        } else {
            line_main + spacing + main
        };
        if max_main.is_some_and(|limit| required > limit) && line_items > 0 {
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
    Dimension::new(total_main, total_minor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_layout_creation() {
        let layout = FlowLayout::new();
        let measured = layout.preferred_measurement(
            FigureId::from(slotmap::KeyData::from_ffi(0)),
            MeasureConstraints::bounded(800.0, 600.0).unwrap(),
            &LayoutSnapshot::new(&MockLayoutContext::new()),
        );
        assert_eq!(measured.size(), Dimension::ZERO);
    }

    #[test]
    fn test_flow_layout_with_direction() {
        let layout = FlowLayout::with_direction(FlowDirection::Vertical);
        assert_eq!(layout.direction, FlowDirection::Vertical);
    }

    #[test]
    fn test_flow_layout_with_spacing() {
        let layout = FlowLayout::new().with_spacing(20.0).with_row_spacing(15.0);
        // 通过 preferred_measurement 间接验证
        let _ = layout.preferred_measurement(
            FigureId::from(slotmap::KeyData::from_ffi(0)),
            MeasureConstraints::bounded(800.0, 600.0).unwrap(),
            &LayoutSnapshot::new(&MockLayoutContext::new()),
        );
    }
}

/// Mock LayoutContext for testing
#[cfg(test)]
struct MockLayoutContext {
    children: Vec<(FigureId, Rectangle)>,
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
    fn get_children(&self, _parent_id: FigureId) -> Vec<(FigureId, Rectangle)> {
        self.children.clone()
    }

    fn get_constraint(&self, _child_id: FigureId) -> Option<&dyn super::LayoutConstraint> {
        None
    }

    fn preferred_measurement(
        &self,
        _figure_id: FigureId,
        _constraints: MeasureConstraints,
    ) -> FigureMeasurement {
        FigureMeasurement::new(100.0, 100.0, None)
    }

    fn get_container_bounds(&self, _container_id: FigureId) -> Rectangle {
        self.container_bounds
    }
}
