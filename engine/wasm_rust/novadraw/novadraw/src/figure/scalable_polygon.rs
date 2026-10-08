use std::{error::Error, fmt};

use crate::geometry::{Point, PointList, Rectangle};
use crate::render::{LineJoin, NdCanvas, StrokeStyle};
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
    pub fn with_stroke(mut self, color: Color, width: f64, join: LineJoin) -> Self {
        self.stroke_color = color;
        let width = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        self.stroke = self
            .stroke
            .with_width(width)
            .expect("valid stroke width")
            .with_join(join);
        self
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

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
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

    fn paint_figure_in_bounds(&self, gc: &mut NdCanvas, bounds: Rectangle) {
        let points = self.scaled_points(Rectangle::new(0.0, 0.0, bounds.width, bounds.height));
        if points.len() < 3 {
            return;
        }
        gc.begin_path();
        let first = points.get(0).expect("validated polygon point count");
        gc.move_to(first.x(), first.y());
        for point in points.iter().skip(1) {
            gc.line_to(point.x(), point.y());
        }
        gc.close_path();
        let fill = self.fill_color.alpha() > 0.0;
        let stroke = self.stroke_color.alpha() > 0.0 && self.stroke.width() > 0.0;
        if stroke {
            gc.set_stroke(self.stroke.clone());
        }
        match (fill, stroke) {
            (true, true) => gc.fill_and_stroke(),
            (true, false) => gc.fill(),
            (false, true) => gc.stroke(),
            (false, false) => {}
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
    let stretch_x = if source_width > f64::EPSILON {
        available.width / source_width
    } else {
        0.0
    };
    let stretch_y = if source_height > f64::EPSILON {
        available.height / source_height
    } else {
        0.0
    };
    let (scale_x, scale_y) = match mode {
        PolygonScaleMode::Stretch => (stretch_x, stretch_y),
        PolygonScaleMode::PreserveAspect => {
            let scale = match (source_width > f64::EPSILON, source_height > f64::EPSILON) {
                (true, true) => stretch_x.min(stretch_y),
                (true, false) => stretch_x,
                (false, true) => stretch_y,
                (false, false) => 0.0,
            };
            (
                if source_width > f64::EPSILON {
                    scale
                } else {
                    0.0
                },
                if source_height > f64::EPSILON {
                    scale
                } else {
                    0.0
                },
            )
        }
    };
    let mapped_width = source_width * scale_x;
    let mapped_height = source_height * scale_y;
    let offset_x =
        available.x + alignment_offset(available.width - mapped_width, horizontal_alignment);
    let offset_y =
        available.y + alignment_offset(available.height - mapped_height, vertical_alignment);

    template
        .iter()
        .map(|point| {
            Point::new(
                offset_x + (point.x() - template_bounds.x) * scale_x,
                offset_y + (point.y() - template_bounds.y) * scale_y,
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
