//! Owned, validated path clipping.

use std::sync::Arc;

use super::command::{Path, PathOp};
use super::stroke::GraphicsInputError;

/// Winding rule shared by path fills and clips.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// An immutable clip, independent of the Canvas current path.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipPath {
    path: Arc<Path>,
    rule: FillRule,
}

impl ClipPath {
    pub fn try_new(path: &Path, rule: FillRule) -> Result<Self, GraphicsInputError> {
        let mut has_start = false;
        for (index, operation) in path.operations().iter().enumerate() {
            let check = |values: &[f64]| {
                if values.iter().all(|value| value.is_finite()) {
                    Ok(())
                } else {
                    Err(GraphicsInputError::NonFinite {
                        field: "clip path",
                        index: Some(index),
                    })
                }
            };
            if !matches!(operation, PathOp::MoveTo(_)) && !has_start {
                return Err(GraphicsInputError::InvalidPath { index });
            }
            match operation {
                PathOp::MoveTo(point) => {
                    check(&[point.x(), point.y()])?;
                    has_start = true;
                }
                PathOp::LineTo(point) => check(&[point.x(), point.y()])?,
                PathOp::HLineTo(value) | PathOp::VLineTo(value) => check(&[*value])?,
                PathOp::CubicTo(a, b, end) => {
                    check(&[a.x(), a.y(), b.x(), b.y(), end.x(), end.y()])?;
                }
                PathOp::QuadTo(control, end) => {
                    check(&[control.x(), control.y(), end.x(), end.y()])?;
                }
                PathOp::Arc {
                    radii,
                    rotation,
                    dest,
                    ..
                } => {
                    check(&[radii.width, radii.height, *rotation, dest.x(), dest.y()])?;
                    if radii.width < 0.0 || radii.height < 0.0 {
                        return Err(GraphicsInputError::InvalidPath { index });
                    }
                }
                PathOp::Close => {}
            }
        }
        Ok(Self {
            path: Arc::new(path.clone()),
            rule,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn rule(&self) -> FillRule {
        self.rule
    }
}
