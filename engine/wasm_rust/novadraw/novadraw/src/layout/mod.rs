//! 布局管理
//!
//! 提供 LayoutManager 接口和常用布局器实现，参考 Eclipse Draw2D 设计。

mod border_layout;
mod fill_layout;
mod flow_layout;
mod freeform_layout;
mod grid_layout;
mod stack_layout;
mod toolbar_layout;
mod xy_layout;

pub use border_layout::{BorderConstraint, BorderLayout, BorderRegion};
pub use fill_layout::FillLayout;
pub use flow_layout::{FlowDirection, FlowLayout};
pub use freeform_layout::{FreeformConstraint, FreeformConstraintError, FreeformLayout};
pub use grid_layout::{GridAlignment, GridConstraint, GridLayout};
pub use stack_layout::StackLayout;
pub use toolbar_layout::{MinorAlignment, ToolbarLayout, ToolbarOrientation};
pub use xy_layout::{XYConstraint, XYLayout};

use crate::container::viewport::ViewportLayoutEffect;
use crate::geometry::{Dimension, Rectangle};
use crate::{FigureMeasurement, MeasureConstraints, PropertyValue, graph::FigureId};
use std::any::Any;
use std::error::Error;
use std::fmt;

/// 容器施加给直接子节点的布局约束。
///
/// 约束由父 FigureNode 持有；具体 LayoutManager 通过 downcast 读取自己支持的类型。
pub trait LayoutConstraint: Any {
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
}

impl<T: Any> LayoutConstraint for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

/// Read-only layout queries used to construct [`LayoutSnapshot`].
///
/// This remains an internal trait so tests and future frozen scene
/// representations can provide the data. Layout managers only receive the
/// concrete immutable snapshot wrapper and cannot mutate the Figure tree.
pub(crate) trait LayoutContext {
    /// 获取子元素列表
    ///
    /// 返回 (child_id, current_bounds) 列表
    fn get_children(&self, parent_id: FigureId) -> Vec<(FigureId, Rectangle)>;

    /// 获取子元素的布局约束
    fn get_constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint>;

    /// 获取 Figure 的首选测量结果。
    fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> FigureMeasurement;

    /// 获取 Figure 的最小尺寸。
    fn minimum_size(&self, figure_id: FigureId, constraints: MeasureConstraints) -> Dimension {
        self.preferred_measurement(figure_id, constraints).size()
    }

    /// 获取 Figure 的最大尺寸。
    fn maximum_size(&self, _figure_id: FigureId) -> Dimension {
        Dimension::new(f64::INFINITY, f64::INFINITY)
    }

    /// 获取容器 client area 在子节点坐标域中的矩形。
    fn get_container_bounds(&self, container_id: FigureId) -> Rectangle;

    fn get_freeform_extent(&self, _figure_id: FigureId) -> Option<Rectangle> {
        None
    }

    fn get_content_scale(&self, _figure_id: FigureId) -> Option<f64> {
        None
    }
}

/// Immutable view of the scene used by one layout calculation.
#[derive(Clone, Copy)]
pub struct LayoutSnapshot<'a> {
    source: &'a dyn LayoutContext,
}

impl<'a> LayoutSnapshot<'a> {
    pub(crate) fn new(source: &'a dyn LayoutContext) -> Self {
        Self { source }
    }

    pub fn children(&self, parent_id: FigureId) -> Vec<(FigureId, Rectangle)> {
        self.source.get_children(parent_id)
    }

    pub fn constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint> {
        self.source.get_constraint(child_id)
    }

    pub fn constraint_as<C: LayoutConstraint>(
        &self,
        container: FigureId,
        child: FigureId,
    ) -> Result<Option<&C>, LayoutError> {
        let Some(constraint) = self.constraint(child) else {
            return Ok(None);
        };
        constraint.as_any().downcast_ref::<C>().map(Some).ok_or(
            LayoutError::ConstraintTypeMismatch {
                container,
                child,
                expected: std::any::type_name::<C>(),
                actual: constraint.type_name(),
            },
        )
    }

    pub fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> FigureMeasurement {
        self.source.preferred_measurement(figure_id, constraints)
    }

    pub fn minimum_size(&self, figure_id: FigureId, constraints: MeasureConstraints) -> Dimension {
        self.source.minimum_size(figure_id, constraints)
    }

    pub fn maximum_size(&self, figure_id: FigureId) -> Dimension {
        self.source.maximum_size(figure_id)
    }

    pub fn container_bounds(&self, container_id: FigureId) -> Rectangle {
        self.source.get_container_bounds(container_id)
    }

    pub fn freeform_extent(&self, figure_id: FigureId) -> Option<Rectangle> {
        self.source.get_freeform_extent(figure_id)
    }

    pub fn content_scale(&self, figure_id: FigureId) -> Option<f64> {
        self.source.get_content_scale(figure_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutError {
    UnknownFigure {
        figure: FigureId,
    },
    ConstraintTypeMismatch {
        container: FigureId,
        child: FigureId,
        expected: &'static str,
        actual: &'static str,
    },
    UnsupportedConstraint {
        container: FigureId,
        child: FigureId,
        actual: &'static str,
    },
    InvalidChild {
        container: FigureId,
        child: FigureId,
    },
    NonFiniteGeometry {
        figure: FigureId,
    },
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFigure { figure } => {
                write!(f, "unknown Figure ID: {figure:?}")
            }
            Self::ConstraintTypeMismatch {
                container,
                child,
                expected,
                actual,
            } => write!(
                f,
                "layout constraint type mismatch for child {child:?} in {container:?}: expected {expected}, got {actual}"
            ),
            Self::UnsupportedConstraint {
                container,
                child,
                actual,
            } => write!(
                f,
                "layout for {container:?} does not accept constraint {actual} for child {child:?}"
            ),
            Self::InvalidChild { container, child } => {
                write!(f, "{child:?} is not a direct child of {container:?}")
            }
            Self::NonFiniteGeometry { figure } => {
                write!(
                    f,
                    "Figure {figure:?} produced non-finite or negative-size layout geometry"
                )
            }
        }
    }
}

impl Error for LayoutError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutInvalidation {
    Structure,
    Constraint,
    Geometry,
    ExplicitSize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LayoutChange {
    Bounds(FigureId, Rectangle),
    Visibility(FigureId, bool),
    Invalidate(FigureId),
    Property {
        figure: FigureId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
    },
    CoordinateSystemChanged(FigureId),
    Repaint(FigureId),
    RepaintParent(FigureId),
    ViewportEffect(ViewportLayoutEffect),
}

/// Buffered changes produced by a layout calculation.
#[derive(Debug, Default)]
pub struct LayoutOutput {
    pub(crate) changes: Vec<LayoutChange>,
}

impl LayoutOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_child_bounds(&mut self, child: FigureId, bounds: Rectangle) {
        self.changes.push(LayoutChange::Bounds(child, bounds));
    }

    pub fn set_child_visible(&mut self, child: FigureId, visible: bool) {
        self.changes.push(LayoutChange::Visibility(child, visible));
    }

    pub fn invalidate(&mut self, child: FigureId) {
        self.changes.push(LayoutChange::Invalidate(child));
    }

    pub(crate) fn record_property_change(
        &mut self,
        figure: FigureId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
    ) {
        self.changes.push(LayoutChange::Property {
            figure,
            property,
            old_value,
            new_value,
        });
    }

    pub(crate) fn coordinate_system_changed(&mut self, figure: FigureId) {
        self.changes
            .push(LayoutChange::CoordinateSystemChanged(figure));
    }

    pub(crate) fn repaint(&mut self, figure: FigureId) {
        self.changes.push(LayoutChange::Repaint(figure));
    }

    pub(crate) fn repaint_parent(&mut self, figure: FigureId) {
        self.changes.push(LayoutChange::RepaintParent(figure));
    }

    pub(crate) fn set_viewport_effect(&mut self, effect: ViewportLayoutEffect) {
        self.changes.push(LayoutChange::ViewportEffect(effect));
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// 布局管理器 trait
///
/// 参考 draw2d: LayoutManager
/// 用于计算和设置子元素的位置。
pub trait LayoutManager {
    /// Validates one parent-owned child constraint before a Runtime transaction commits it.
    ///
    /// Managers that consume constraints must override this method and accept every concrete
    /// compatibility type that their layout implementation can read.
    fn validate_constraint(
        &self,
        container: FigureId,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        Err(LayoutError::UnsupportedConstraint {
            container,
            child,
            actual: constraint.type_name(),
        })
    }

    /// 获取首选测量结果。
    ///
    /// 对应 draw2d: getPreferredSize(IFigure, int, int)，但使用结构化约束和结果。
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement;

    /// 获取最小大小
    ///
    /// 对应 draw2d: getMinimumSize(IFigure, int, int)
    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension;

    /// 执行布局
    ///
    /// 对应 draw2d: layout(IFigure)
    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError>;

    fn invalidate(&mut self, _reason: LayoutInvalidation) {}

    fn requires_valid_children_before_layout(&self) -> bool {
        false
    }
}
