//! XY 布局器
//!
//! 参考 draw2d: XYLayout
//! 使用约束（Rectangle）定位每个子元素。

use super::{LayoutConstraint, LayoutError, LayoutManager, LayoutOutput, LayoutSnapshot};
use crate::geometry::{Dimension, Rectangle};
use crate::{FigureMeasurement, MeasureConstraints, graph::FigureId};

/// XY 布局约束
///
/// 对应 draw2d 中的 Rectangle 约束
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XYConstraint {
    /// 位置 x
    pub x: f64,
    /// 位置 y
    pub y: f64,
    /// 宽度，-1 表示使用首选宽度
    pub width: f64,
    /// 高度，-1 表示使用首选高度
    pub height: f64,
}

impl XYConstraint {
    /// 从 Rectangle 创建约束
    pub fn from_rect(rect: Rectangle) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }

    /// 创建指定位置的约束
    pub fn at(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            width: -1.0,
            height: -1.0,
        }
    }

    /// 创建指定位置的约束
    pub fn at_size(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

impl Default for XYConstraint {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: -1.0,
            height: -1.0,
        }
    }
}

/// XY 布局器
///
/// 参考 draw2d: XYLayout
/// 根据每个子元素的约束 Rectangle 来定位和设置大小。
#[derive(Debug, Clone)]
pub struct XYLayout;

impl XYLayout {
    /// 创建新的 XYLayout
    pub fn new() -> Self {
        Self
    }

    fn measure(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
        minimum: bool,
    ) -> Dimension {
        snapshot
            .children(container)
            .into_iter()
            .filter_map(|(child, _)| {
                let constraint = xy_constraint(snapshot, container, child).ok().flatten()?;
                let child_constraints = MeasureConstraints::new(
                    (constraint.width >= 0.0)
                        .then_some(constraint.width)
                        .or(constraints.max_width()),
                    (constraint.height >= 0.0)
                        .then_some(constraint.height)
                        .or(constraints.max_height()),
                )
                .ok()?;
                let intrinsic = if minimum {
                    snapshot.minimum_size(child, child_constraints)
                } else {
                    snapshot
                        .preferred_measurement(child, child_constraints)
                        .size()
                };
                let width = if constraint.width < 0.0 {
                    intrinsic.width
                } else {
                    constraint.width
                };
                let height = if constraint.height < 0.0 {
                    intrinsic.height
                } else {
                    constraint.height
                };
                Some(Dimension::new(constraint.x + width, constraint.y + height))
            })
            .fold(Dimension::ZERO, |size, child_extent| {
                Dimension::new(
                    size.width.max(child_extent.width),
                    size.height.max(child_extent.height),
                )
            })
    }
}

impl Default for XYLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutManager for XYLayout {
    fn validate_constraint(
        &self,
        container: FigureId,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        if let Some(constraint) = constraint.as_any().downcast_ref::<XYConstraint>() {
            if constraint.x.is_finite()
                && constraint.y.is_finite()
                && constraint.width.is_finite()
                && constraint.height.is_finite()
            {
                return Ok(());
            }
            return Err(LayoutError::NonFiniteGeometry { figure: child });
        }
        if let Some(constraint) = constraint.as_any().downcast_ref::<Rectangle>() {
            if constraint.x.is_finite()
                && constraint.y.is_finite()
                && constraint.width.is_finite()
                && constraint.height.is_finite()
            {
                return Ok(());
            }
            return Err(LayoutError::NonFiniteGeometry { figure: child });
        }
        Err(LayoutError::ConstraintTypeMismatch {
            container,
            child,
            expected: "XYConstraint or Rectangle",
            actual: constraint.type_name(),
        })
    }

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
        let children = snapshot.children(container);
        if children.is_empty() {
            return Ok(());
        }

        // LayoutSnapshot exposes the available rectangle in child-content coordinates.
        let container_bounds = snapshot.container_bounds(container);
        let offset_x = container_bounds.x;
        let offset_y = container_bounds.y;

        for (child_id, _) in children {
            if let Some(constraint) = xy_constraint(snapshot, container, child_id)? {
                let child_constraints = MeasureConstraints::new(
                    (constraint.width >= 0.0).then_some(constraint.width),
                    (constraint.height >= 0.0).then_some(constraint.height),
                )
                .expect("validated XYConstraint dimensions produce valid measurement constraints");
                let preferred = snapshot
                    .preferred_measurement(child_id, child_constraints)
                    .size();
                let width = if constraint.width < 0.0 {
                    preferred.width
                } else {
                    constraint.width
                };
                let height = if constraint.height < 0.0 {
                    preferred.height
                } else {
                    constraint.height
                };
                let new_bounds = Rectangle::new(
                    constraint.x + offset_x,
                    constraint.y + offset_y,
                    width,
                    height,
                );
                // 应用约束作为新的 bounds
                out.set_child_bounds(child_id, new_bounds);
            }
        }
        Ok(())
    }
}

fn xy_constraint(
    snapshot: &LayoutSnapshot<'_>,
    container: FigureId,
    child_id: FigureId,
) -> Result<Option<Rectangle>, LayoutError> {
    let Some(constraint) = snapshot.constraint(child_id) else {
        return Ok(None);
    };
    if let Some(rect) = constraint.as_any().downcast_ref::<Rectangle>() {
        return Ok(Some(*rect));
    }
    snapshot
        .constraint_as::<XYConstraint>(container, child_id)
        .map(|constraint| {
            constraint.map(|constraint| {
                Rectangle::new(
                    constraint.x,
                    constraint.y,
                    constraint.width,
                    constraint.height,
                )
            })
        })
}
