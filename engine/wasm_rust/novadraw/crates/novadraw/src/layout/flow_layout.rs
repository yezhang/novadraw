//! Flow 布局器
//!
//! 参考 draw2d: FlowLayout
//! 按顺序排列子元素，自动换行。

use super::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery};
use crate::geometry::{Dimension, Rectangle};
use crate::{FigureMeasurement, MeasureConstraints, graph::FigureId};

/// Flow 布局方向
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FlowDirection {
    /// 水平流动（从左到右，然后换行）
    #[default]
    Horizontal,
    /// 垂直流动（从上到下，然后换列）
    Vertical,
}

/// Alignment used by Draw2D FlowLayout on its major and minor axes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FlowAlignment {
    /// Top or left edge.
    #[default]
    Start,
    /// Center of the available axis.
    Center,
    /// Bottom or right edge.
    End,
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
    major_alignment: FlowAlignment,
    minor_alignment: FlowAlignment,
    stretch_minor_axis: bool,
}

impl FlowLayout {
    /// 创建新的 FlowLayout（水平方向）
    pub fn new() -> Self {
        Self {
            direction: FlowDirection::Horizontal,
            spacing: 5.0,
            row_spacing: 5.0,
            major_alignment: FlowAlignment::Start,
            minor_alignment: FlowAlignment::Start,
            stretch_minor_axis: false,
        }
    }

    /// 创建指定方向的 FlowLayout
    pub fn with_direction(direction: FlowDirection) -> Self {
        Self {
            direction,
            ..Self::new()
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

    /// Aligns each wrapped row or column on the major axis.
    pub fn with_major_alignment(mut self, alignment: FlowAlignment) -> Self {
        self.major_alignment = alignment;
        self
    }

    /// Aligns children within each wrapped row or column.
    pub fn with_minor_alignment(mut self, alignment: FlowAlignment) -> Self {
        self.minor_alignment = alignment;
        self
    }

    /// Stretches children to the row height or column width.
    pub fn with_stretch_minor_axis(mut self, stretch: bool) -> Self {
        self.stretch_minor_axis = stretch;
        self
    }

    fn measure(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        let child_constraints = match self.direction {
            FlowDirection::Horizontal => MeasureConstraints::new(constraints.max_width(), None),
            FlowDirection::Vertical => MeasureConstraints::new(None, constraints.max_height()),
        }
        .expect("axis selection preserves validated measurement constraints");
        let sizes = snapshot
            .children(container)?
            .into_iter()
            .map(|(child, _)| {
                snapshot
                    .preferred_measurement(child, child_constraints)
                    .map(|measurement| measurement.size())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match self.direction {
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
        })
    }

    /// 布局计算（内部方法）
    fn perform_layout(
        &self,
        container: FigureId,
        snapshot: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let children = snapshot.children(container)?;
        if children.is_empty() {
            return Ok(());
        }

        let container_bounds = snapshot.container_bounds(container)?;

        self.layout_oriented(container_bounds, &children, snapshot, out)
    }

    fn layout_oriented(
        &self,
        area: Rectangle,
        children: &[(FigureId, Rectangle)],
        snapshot: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let horizontal = self.direction == FlowDirection::Horizontal;
        let available_main = if horizontal { area.width } else { area.height };
        let main_origin = if horizontal { area.x } else { area.y };
        let minor_origin = if horizontal { area.y } else { area.x };
        let measurement_constraints = if horizontal {
            MeasureConstraints::width(area.width)
        } else {
            MeasureConstraints::height(area.height)
        }
        .expect("container extent is valid layout geometry");

        let mut lines = Vec::<FlowLine>::new();
        let mut line = FlowLine::default();
        for (child, _) in children {
            let measured = snapshot.preferred_measurement(*child, measurement_constraints)?;
            let (main, minor) = if horizontal {
                (measured.width, measured.height)
            } else {
                (measured.height, measured.width)
            };
            let required = if line.items.is_empty() {
                main
            } else {
                line.main + self.spacing + main
            };
            if !line.items.is_empty() && required > available_main {
                lines.push(line);
                line = FlowLine::default();
            }
            if !line.items.is_empty() {
                line.main += self.spacing;
            }
            line.items.push(FlowItem {
                figure: *child,
                main,
                minor,
            });
            line.main += main;
            line.minor = line.minor.max(minor);
        }
        if !line.items.is_empty() {
            lines.push(line);
        }

        let mut line_minor = minor_origin;
        for line in lines {
            let major_offset = aligned_offset(available_main, line.main, self.major_alignment);
            let mut item_main = main_origin + major_offset;
            for item in line.items {
                let item_minor = if self.stretch_minor_axis {
                    line.minor
                } else {
                    item.minor
                };
                let minor_offset = aligned_offset(line.minor, item_minor, self.minor_alignment);
                let bounds = if horizontal {
                    Rectangle::new(item_main, line_minor + minor_offset, item.main, item_minor)
                } else {
                    Rectangle::new(line_minor + minor_offset, item_main, item_minor, item.main)
                };
                out.set_child_bounds(item.figure, bounds);
                item_main += item.main + self.spacing;
            }
            line_minor += line.minor + self.row_spacing;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct FlowItem {
    figure: FigureId,
    main: f64,
    minor: f64,
}

#[derive(Default)]
struct FlowLine {
    items: Vec<FlowItem>,
    main: f64,
    minor: f64,
}

fn aligned_offset(available: f64, used: f64, alignment: FlowAlignment) -> f64 {
    match alignment {
        FlowAlignment::Start => 0.0,
        FlowAlignment::Center => {
            let offset = (available - used) / 2.0;
            if available.fract() == 0.0 && used.fract() == 0.0 {
                offset.trunc()
            } else {
                offset
            }
        }
        FlowAlignment::End => available - used,
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
        snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        let size = self.measure(container, constraints, snapshot)?;
        Ok(FigureMeasurement::new(size.width, size.height, None))
    }

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        self.measure(container, constraints, snapshot)
    }

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.perform_layout(container, snapshot, out)
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

/// Mock LayoutSceneRead for testing
#[cfg(test)]
struct MockLayoutSceneRead {
    children: Vec<(FigureId, Rectangle)>,
    container_bounds: Rectangle,
}

#[cfg(test)]
impl MockLayoutSceneRead {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            container_bounds: Rectangle::new(0.0, 0.0, 800.0, 600.0),
        }
    }
}

#[cfg(test)]
impl super::LayoutSceneRead for MockLayoutSceneRead {
    fn get_children(
        &self,
        _parent_id: FigureId,
    ) -> Result<Vec<(FigureId, Rectangle)>, LayoutError> {
        Ok(self.children.clone())
    }

    fn get_constraint(
        &self,
        _child_id: FigureId,
    ) -> Result<Option<&dyn super::LayoutConstraint>, LayoutError> {
        Ok(None)
    }

    fn preferred_measurement(
        &self,
        figure_id: FigureId,
        _constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        let bounds = self
            .children
            .iter()
            .find_map(|(child, bounds)| (*child == figure_id).then_some(*bounds))
            .unwrap_or(Rectangle::new(0.0, 0.0, 100.0, 100.0));
        Ok(FigureMeasurement::new(bounds.width, bounds.height, None))
    }

    fn get_container_bounds(&self, _container_id: FigureId) -> Result<Rectangle, LayoutError> {
        Ok(self.container_bounds)
    }
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
            &LayoutSceneQuery::new(&MockLayoutSceneRead::new()),
        );
        assert_eq!(measured.unwrap().size(), Dimension::ZERO);
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
            &LayoutSceneQuery::new(&MockLayoutSceneRead::new()),
        );
    }

    #[test]
    fn major_and_minor_alignment_match_draw2d_ordered_layout_semantics() {
        let container = FigureId::from(slotmap::KeyData::from_ffi(1));
        let first = FigureId::from(slotmap::KeyData::from_ffi(2));
        let second = FigureId::from(slotmap::KeyData::from_ffi(3));
        let scene = MockLayoutSceneRead {
            children: vec![
                (first, Rectangle::new(0.0, 0.0, 50.0, 20.0)),
                (second, Rectangle::new(0.0, 0.0, 50.0, 40.0)),
            ],
            container_bounds: Rectangle::new(0.0, 0.0, 300.0, 120.0),
        };
        let mut layout = FlowLayout::new()
            .with_major_alignment(FlowAlignment::End)
            .with_minor_alignment(FlowAlignment::End);
        let mut output = LayoutOutput::new();

        layout
            .layout(container, &LayoutSceneQuery::new(&scene), &mut output)
            .unwrap();

        assert_eq!(
            output.changes,
            vec![
                super::super::LayoutChange::Bounds(first, Rectangle::new(195.0, 20.0, 50.0, 20.0)),
                super::super::LayoutChange::Bounds(second, Rectangle::new(250.0, 0.0, 50.0, 40.0)),
            ]
        );
    }

    #[test]
    fn stretch_minor_axis_uses_the_wrapped_line_extent() {
        let container = FigureId::from(slotmap::KeyData::from_ffi(11));
        let first = FigureId::from(slotmap::KeyData::from_ffi(12));
        let second = FigureId::from(slotmap::KeyData::from_ffi(13));
        let scene = MockLayoutSceneRead {
            children: vec![
                (first, Rectangle::new(0.0, 0.0, 50.0, 20.0)),
                (second, Rectangle::new(0.0, 0.0, 50.0, 40.0)),
            ],
            container_bounds: Rectangle::new(0.0, 0.0, 300.0, 120.0),
        };
        let mut layout = FlowLayout::new().with_stretch_minor_axis(true);
        let mut output = LayoutOutput::new();

        layout
            .layout(container, &LayoutSceneQuery::new(&scene), &mut output)
            .unwrap();

        assert_eq!(
            output.changes,
            vec![
                super::super::LayoutChange::Bounds(first, Rectangle::new(0.0, 0.0, 50.0, 40.0)),
                super::super::LayoutChange::Bounds(second, Rectangle::new(55.0, 0.0, 50.0, 40.0)),
            ]
        );
    }
}
