use std::{error::Error, fmt};

use crate::geometry::{Point, PointList, Rectangle};
use crate::graphics::{GraphicsError, PaintContext, Path};
use crate::render::{LineJoin, StrokeStyle};
use crate::{Alignment, Color, Figure, FigureStyle};

const DEFAULT_STROKE_WIDTH: f64 = 2.0;

/// How a polygon template maps into the Figure bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PolygonScaleMode {
    /// Scale each axis independently to fill the available bounds.
    #[default]
    Stretch,
    /// Preserve the template aspect ratio and align any remaining space.
    PreserveAspect,
}

/// Failure to construct a scalable polygon.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalablePolygonError {
    /// Template points contain a non-finite coordinate.
    NonFiniteTemplate,
    /// Initial bounds are non-finite or have a negative extent.
    InvalidBounds,
}

impl fmt::Display for ScalablePolygonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteTemplate => {
                formatter.write_str("scalable polygon template points must be finite")
            }
            Self::InvalidBounds => formatter
                .write_str("scalable polygon bounds must be finite with non-negative dimensions"),
        }
    }
}

impl Error for ScalablePolygonError {}

/// Polygon whose canonical template is mapped into its current Figure bounds.
#[derive(Clone)]
pub struct ScalablePolygonFigure {
    bounds: Rectangle,
    template: PointList,
    scale_mode: PolygonScaleMode,
    horizontal_alignment: Alignment,
    vertical_alignment: Alignment,
    fill_color: Color,
    stroke_color: Color,
    stroke: StrokeStyle,
}

impl ScalablePolygonFigure {
    /// Creates a scalable polygon with validated bounds and template.
    pub fn new(bounds: Rectangle, template: PointList) -> Result<Self, ScalablePolygonError> {
        validate_bounds(bounds)?;
        validate_template(&template)?;
        Ok(Self {
            bounds,
            template,
            scale_mode: PolygonScaleMode::Stretch,
            horizontal_alignment: Alignment::Center,
            vertical_alignment: Alignment::Center,
            fill_color: Color::from_hex("#3498db").expect("valid color literal"),
            stroke_color: Color::from_hex("#2c3e50").expect("valid color literal"),
            stroke: StrokeStyle::default()
                .with_width(DEFAULT_STROKE_WIDTH)
                .expect("valid default stroke"),
        })
    }

    /// Sets the construction-time scale mode.
    pub fn with_scale_mode(mut self, mode: PolygonScaleMode) -> Self {
        self.scale_mode = mode;
        self
    }

    /// Sets construction-time alignment.
    pub fn with_alignment(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.horizontal_alignment = horizontal;
        self.vertical_alignment = vertical;
        self
    }

    /// Sets fill color.
    pub fn with_fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    /// Sets outline color, width, and join.
    pub fn with_stroke(
        mut self,
        color: Color,
        width: f64,
        join: LineJoin,
    ) -> Result<Self, crate::graphics::GraphicsInputError> {
        self.stroke = self.stroke.with_width(width)?.with_join(join);
        self.stroke_color = color;
        Ok(self)
    }

    /// Reserves the complete stroke envelope inside the assigned bounds.
    pub fn with_stroke_style(mut self, stroke: StrokeStyle) -> Self {
        self.stroke = stroke;
        self
    }

    /// Returns template points mapped into the supplied node-local bounds.
    pub fn scaled_points(&self, bounds: Rectangle) -> PointList {
        PointList::from_points(map_template(
            &self.template,
            bounds,
            self.scale_mode,
            self.horizontal_alignment,
            self.vertical_alignment,
            self.stroke.visual_outset(),
        ))
    }

    pub(crate) fn template(&self) -> &PointList {
        &self.template
    }

    pub(crate) fn scale_mode(&self) -> PolygonScaleMode {
        self.scale_mode
    }

    pub(crate) fn alignment(&self) -> (Alignment, Alignment) {
        (self.horizontal_alignment, self.vertical_alignment)
    }

    pub(crate) fn replace_template(&mut self, template: PointList) {
        self.template = template;
    }

    pub(crate) fn replace_scale_mode(&mut self, mode: PolygonScaleMode) {
        self.scale_mode = mode;
    }

    pub(crate) fn replace_alignment(&mut self, horizontal: Alignment, vertical: Alignment) {
        self.horizontal_alignment = horizontal;
        self.vertical_alignment = vertical;
    }
}

impl super::Bounded for ScalablePolygonFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), super::ShapeGeometryError> {
        self.bounds =
            super::ShapeGeometryError::validate_bounds(Rectangle::new(x, y, width, height))?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "ScalablePolygonFigure"
    }
}

impl Figure for ScalablePolygonFigure {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }

    fn name(&self) -> &'static str {
        "ScalablePolygonFigure"
    }

    fn initial_style(&self) -> FigureStyle {
        FigureStyle {
            foreground: Some(self.stroke_color),
            background: Some(self.fill_color),
            ..FigureStyle::default()
        }
    }

    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), GraphicsError> {
        let points = self.scaled_points(Rectangle::new(0.0, 0.0, bounds.width, bounds.height));
        if points.len() < 3 {
            return Ok(());
        }
        let mut path = Path::new();
        let first = points.get(0).expect("validated polygon point count");
        path.move_to(first.x(), first.y());
        for point in points.iter().skip(1) {
            path.line_to(point.x(), point.y());
        }
        path.close();
        let fill = self.fill_color.alpha() > 0.0;
        let stroke = self.stroke_color.alpha() > 0.0 && self.stroke.width() > 0.0;
        if stroke {
            context.set_stroke_style(self.stroke.clone());
        }
        match (fill, stroke) {
            (true, true) => {
                context.fill_path(&path)?;
                context.stroke_path(&path)
            }
            (true, false) => context.fill_path(&path),
            (false, true) => context.stroke_path(&path),
            (false, false) => Ok(()),
        }
    }

    fn precise_hit(&self, x: f64, y: f64, bounds: Rectangle) -> bool {
        let points = self.scaled_points(Rectangle::new(0.0, 0.0, bounds.width, bounds.height));
        if points.len() < 3 {
            return false;
        }
        if points.iter().enumerate().any(|(index, point)| {
            let next = points
                .get((index + 1) % points.len())
                .expect("closed polygon point");
            super::polyline::point_segment_distance_squared(
                x,
                y,
                point.x(),
                point.y(),
                next.x(),
                next.y(),
            ) <= f64::EPSILON
        }) {
            return true;
        }
        let mut inside = false;
        let mut previous = points.len() - 1;
        for current in 0..points.len() {
            let current_point = points.get(current).expect("polygon point");
            let previous_point = points.get(previous).expect("polygon point");
            if (current_point.y() > y) != (previous_point.y() > y)
                && x < (previous_point.x() - current_point.x()) * (y - current_point.y())
                    / (previous_point.y() - current_point.y())
                    + current_point.x()
            {
                inside = !inside;
            }
            previous = current;
        }
        inside
    }
}

fn map_template(
    template: &PointList,
    bounds: Rectangle,
    mode: PolygonScaleMode,
    horizontal_alignment: Alignment,
    vertical_alignment: Alignment,
    outset: f64,
) -> Vec<Point> {
    let Some(template_bounds) = template.bounds() else {
        return Vec::new();
    };
    let available = Rectangle::new(
        outset,
        outset,
        (bounds.width - outset * 2.0).max(0.0),
        (bounds.height - outset * 2.0).max(0.0),
    );
    let source_width = template_bounds.width;
    let source_height = template_bounds.height;
    let mut mapped_width = if source_width > f64::EPSILON {
        available.width
    } else {
        0.0
    };
    let mut mapped_height = if source_height > f64::EPSILON {
        available.height
    } else {
        0.0
    };
    if mode == PolygonScaleMode::PreserveAspect
        && source_width > f64::EPSILON
        && source_height > f64::EPSILON
    {
        // Compare using a ratio no greater than one. Dividing target dimensions
        // by a tiny template span first can overflow despite finite final points.
        if source_width >= source_height {
            let ratio = source_height / source_width;
            if available.width * ratio > available.height {
                mapped_width = available.height / ratio;
                mapped_height = available.height;
            } else {
                mapped_height = available.width * ratio;
            }
        } else {
            let ratio = source_width / source_height;
            if available.height * ratio > available.width {
                mapped_height = available.width / ratio;
                mapped_width = available.width;
            } else {
                mapped_width = available.height * ratio;
            }
        }
    }
    let offset_x =
        available.x + alignment_offset(available.width - mapped_width, horizontal_alignment);
    let offset_y =
        available.y + alignment_offset(available.height - mapped_height, vertical_alignment);

    template
        .iter()
        .map(|point| {
            Point::new(
                offset_x
                    + if source_width > f64::EPSILON {
                        (point.x() - template_bounds.x) / source_width * mapped_width
                    } else {
                        0.0
                    },
                offset_y
                    + if source_height > f64::EPSILON {
                        (point.y() - template_bounds.y) / source_height * mapped_height
                    } else {
                        0.0
                    },
            )
        })
        .collect()
}

fn alignment_offset(remaining: f64, alignment: Alignment) -> f64 {
    match alignment {
        Alignment::Start => 0.0,
        Alignment::Center => remaining / 2.0,
        Alignment::End => remaining,
    }
}

fn validate_template(template: &PointList) -> Result<(), ScalablePolygonError> {
    if template
        .iter()
        .any(|point| !point.x().is_finite() || !point.y().is_finite())
        || template
            .bounds()
            .is_some_and(|bounds| super::ShapeGeometryError::validate_bounds(bounds).is_err())
    {
        Err(ScalablePolygonError::NonFiniteTemplate)
    } else {
        Ok(())
    }
}

fn validate_bounds(bounds: Rectangle) -> Result<(), ScalablePolygonError> {
    if bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width >= 0.0
        && bounds.height >= 0.0
        && (bounds.x + bounds.width).is_finite()
        && (bounds.y + bounds.height).is_finite()
    {
        Ok(())
    } else {
        Err(ScalablePolygonError::InvalidBounds)
    }
}
