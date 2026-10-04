//! Submission preflight for portable f32 rendering inputs.

use super::command::{Path, PathOp};
use super::path_geometry::{NormalizedPathOp, for_each_normalized};
use super::{GraphicsInputError, Paint, RenderCommand, RenderCommandKind};
use crate::geometry::{Affine2D, Point};

/// A rejected command; neither a frame nor its resources have been accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidGraphicsInput {
    pub command_index: usize,
    pub reason: GraphicsInputError,
}

impl std::fmt::Display for InvalidGraphicsInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "graphics command {}: {}",
            self.command_index, self.reason
        )
    }
}

impl std::error::Error for InvalidGraphicsInput {}

fn scalar(value: f64, field: &'static str) -> Result<f32, GraphicsInputError> {
    let value = value as f32;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(GraphicsInputError::DeviceValueOverflow { field })
    }
}

struct DeviceSpace {
    affine: [f32; 6],
    scale: f64,
}

impl DeviceSpace {
    fn new(transform: Affine2D, scale: f64) -> Result<Self, GraphicsInputError> {
        if scale <= 0.0 || scalar(scale, "surface scale")? == 0.0 {
            return Err(GraphicsInputError::InvalidDeviceScale);
        }
        let [a, b, c, d, e, f] = transform.coeffs();
        let mut affine = [0.0; 6];
        for (output, value) in affine.iter_mut().zip([a, b, c, d, e * scale, f * scale]) {
            *output = scalar(value, "transform")?;
        }
        Ok(Self { affine, scale })
    }

    fn scaled_point(&self, point: Point) -> Result<[f32; 2], GraphicsInputError> {
        let x = scalar(point.x(), "geometry")?;
        let y = scalar(point.y(), "geometry")?;
        let [a, b, c, d, e, f] = self.affine;
        let result = [a * x + c * y + e, b * x + d * y + f];
        for value in result {
            scalar(f64::from(value), "transformed geometry")?;
        }
        Ok(result)
    }

    fn point(&self, point: Point) -> Result<[f32; 2], GraphicsInputError> {
        self.scaled_point(Point::new(point.x() * self.scale, point.y() * self.scale))
    }

    fn path(&self, path: &Path) -> Result<(), GraphicsInputError> {
        // Reject invalid arc inputs before invoking the curve normalizer.
        for op in path.operations() {
            if let PathOp::Arc {
                radii, rotation, ..
            } = op
            {
                scalar(radii.width * self.scale, "arc radius")?;
                scalar(radii.height * self.scale, "arc radius")?;
                scalar(*rotation, "arc rotation")?;
            }
        }
        let mut result = Ok(());
        for_each_normalized(path.operations(), self.scale, |op| {
            let points = match op {
                NormalizedPathOp::MoveTo(p) | NormalizedPathOp::LineTo(p) => [Some(p), None, None],
                NormalizedPathOp::CubicTo(c) => [Some(c.control1), Some(c.control2), Some(c.end)],
                NormalizedPathOp::QuadTo { control, end } => [Some(control), Some(end), None],
                NormalizedPathOp::Close => [None; 3],
            };
            for point in points.into_iter().flatten() {
                if result.is_ok() {
                    result = self.scaled_point(point).map(|_| ());
                }
            }
        });
        result
    }

    fn command(&self, command: &RenderCommandKind) -> Result<(), GraphicsInputError> {
        for paint in command.paints() {
            if let Paint::LinearGradient(gradient) = paint {
                let [a, b, c, d, _, _] = self.affine;
                let determinant = f64::from(a) * f64::from(d) - f64::from(b) * f64::from(c);
                if determinant == 0.0 {
                    return Err(GraphicsInputError::DegenerateDeviceGradient);
                }
                for coefficient in [a, b, c, d] {
                    scalar(
                        f64::from(coefficient) / determinant,
                        "gradient transform inverse",
                    )?;
                }
                let start = self.point(gradient.start())?;
                let end = self.point(gradient.end())?;
                let local = |p: Point| [(p.x() * self.scale) as f32, (p.y() * self.scale) as f32];
                if start == end || local(gradient.start()) == local(gradient.end()) {
                    return Err(GraphicsInputError::DegenerateDeviceGradient);
                }
                let delta = [
                    (gradient.end().x() - gradient.start().x()) * self.scale,
                    (gradient.end().y() - gradient.start().y()) * self.scale,
                ];
                let norm = delta[0] * delta[0] + delta[1] * delta[1];
                let inverse = [
                    scalar(delta[0] / norm, "gradient inverse")?,
                    scalar(delta[1] / norm, "gradient inverse")?,
                ];
                if inverse == [0.0; 2] {
                    return Err(GraphicsInputError::DegenerateDeviceGradient);
                }
            }
        }
        if let Some(stroke) = command.stroke() {
            scalar(stroke.width() * self.scale, "stroke width")?;
            scalar(stroke.miter_limit(), "miter limit")?;
            scalar(stroke.normalized_dash_offset() * self.scale, "dash offset")?;
            for length in stroke.dash_lengths().iter() {
                let length = scalar(length * self.scale, "dash length")?;
                if stroke.width() > 0.0 && length <= 0.0 {
                    return Err(GraphicsInputError::DeviceValueUnderflow {
                        field: "dash length",
                    });
                }
            }
            let [a, b, c, d, _, _] = self.affine;
            let extent = stroke.visual_outset() * self.scale;
            scalar(
                extent * (f64::from(a.abs()) + f64::from(c.abs())),
                "stroke extent",
            )?;
            scalar(
                extent * (f64::from(b.abs()) + f64::from(d.abs())),
                "stroke extent",
            )?;
        }
        match command {
            RenderCommandKind::ClipPath { clip } => self.path(clip.path())?,
            RenderCommandKind::FillPath { path, .. }
            | RenderCommandKind::StrokePath { path, .. } => self.path(path)?,
            RenderCommandKind::FillRect { rect, .. }
            | RenderCommandKind::StrokeRect { rect, .. }
            | RenderCommandKind::ClearRect { rect, .. }
            | RenderCommandKind::Clip { rect } => {
                for x in [rect.x, rect.x + rect.width] {
                    for y in [rect.y, rect.y + rect.height] {
                        self.point(Point::new(x, y))?;
                    }
                }
            }
            RenderCommandKind::Ellipse { cx, cy, rx, ry, .. } => {
                for x in [cx - rx, cx + rx] {
                    for y in [cy - ry, cy + ry] {
                        self.point(Point::new(x, y))?;
                    }
                }
            }
            RenderCommandKind::Line { p1, p2, .. } => {
                self.point(*p1)?;
                self.point(*p2)?;
            }
            RenderCommandKind::Polyline { points, .. } => {
                for point in points.iter() {
                    self.point(*point)?;
                }
            }
            RenderCommandKind::DrawGlyphRun { run, origin, .. } => {
                scalar(f64::from(run.font_size) * self.scale, "glyph size")?;
                if let Some(skew) = run.skew_degrees {
                    scalar(f64::from(skew.to_radians().tan()), "glyph skew")?;
                }
                for glyph in &run.glyphs {
                    // Match the glyph builder's f32 origin addition before DPI multiplication.
                    self.scaled_point(Point::new(
                        f64::from((origin.x() as f32 + glyph.x) * self.scale as f32),
                        f64::from((origin.y() as f32 + glyph.y) * self.scale as f32),
                    ))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Validates without changing Canvas state, resource ownership or backend state.
pub fn validate_graphics_input(
    commands: &[RenderCommand],
    scale: f64,
) -> Result<(), InvalidGraphicsInput> {
    let mut transform = Affine2D::IDENTITY;
    let mut stack = Vec::new();
    for (command_index, command) in commands.iter().enumerate() {
        match &command.kind {
            RenderCommandKind::PushState => stack.push(transform),
            RenderCommandKind::RestoreState => {
                if let Some(saved) = stack.last() {
                    transform = *saved;
                }
            }
            RenderCommandKind::PopState => {
                if let Some(saved) = stack.pop() {
                    transform = saved;
                }
            }
            RenderCommandKind::ConcatTransform { matrix } => {
                transform = transform.post_concat(*matrix)
            }
            RenderCommandKind::SetTransform { matrix } => transform = *matrix,
            RenderCommandKind::ResetTransform => transform = Affine2D::IDENTITY,
            _ => {}
        }
        DeviceSpace::new(transform, scale)
            .and_then(|device| device.command(&command.kind))
            .map_err(|reason| InvalidGraphicsInput {
                command_index,
                reason,
            })?;
    }
    Ok(())
}
