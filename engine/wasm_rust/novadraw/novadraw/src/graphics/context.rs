use super::*;
use crate::render::{NdCanvas, ResourceUpdate};
use crate::runtime::ResourceRegistry;
use crate::text::{
    FontDescriptor, FontMetrics, MeasureContext, TextConstraints, TextError, TextLayout,
    TextMetrics, TextSystem,
};
use crate::{Point, Rectangle};

#[derive(Clone, Debug, PartialEq)]
pub enum GraphicsError {
    Input(GraphicsInputError),
    Resource(crate::ResourceError),
    Font(crate::text::outline::FontError),
    ConflictingResource(crate::ResourceId),
    UnbalancedState,
    RecordingBudgetExceeded { limit: usize, actual: usize },
    ImmutableDrawingUnavailable,
    InvalidOpacity,
    Image(crate::render::ImageDrawError),
}
impl std::fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            Self::Font(e) => e.fmt(f),
            Self::ConflictingResource(id) => write!(f, "resource {id:?} has conflicting revisions"),
            Self::UnbalancedState => f.write_str("unbalanced graphics state stack"),
            Self::RecordingBudgetExceeded { limit, actual } => {
                write!(
                    f,
                    "drawing storage {actual} exceeds recording limit {limit}"
                )
            }
            Self::ImmutableDrawingUnavailable => {
                f.write_str("drawing does not provide immutable bounded inputs")
            }
            Self::InvalidOpacity => f.write_str("opacity must be finite and in [0, 1]"),
            Self::Image(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for GraphicsError {}
impl From<GraphicsInputError> for GraphicsError {
    fn from(e: GraphicsInputError) -> Self {
        Self::Input(e)
    }
}

/// Drawing-only view. It cannot clear commands, mutate damage or submit a frame.
pub struct PaintContext<'a> {
    pub(crate) canvas: &'a mut NdCanvas,
    resources: Option<&'a ResourceRegistry>,
    leases: Option<&'a mut Vec<ResourceUpdate>>,
    state_floor: usize,
    figure_scope: bool,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn for_figure(canvas: &'a mut NdCanvas) -> Self {
        Self::for_recorder(canvas, None, None)
    }

    pub(crate) fn for_recorder(
        canvas: &'a mut NdCanvas,
        resources: Option<&'a ResourceRegistry>,
        leases: Option<&'a mut Vec<ResourceUpdate>>,
    ) -> Self {
        let state_floor = canvas.state_depth();
        Self {
            canvas,
            resources,
            leases,
            state_floor,
            figure_scope: true,
        }
    }

    fn reject<T>(&mut self, error: GraphicsError) -> Result<T, GraphicsError> {
        self.canvas.reject_recording(error.clone());
        Err(error)
    }

    pub fn set_antialias(&mut self, mode: AntialiasMode) {
        self.canvas.set_antialias(mode);
    }
    pub fn antialias(&self) -> AntialiasMode {
        self.canvas.antialias()
    }
    pub fn set_text_antialias(&mut self, mode: AntialiasMode) {
        self.canvas.set_text_antialias(mode);
    }
    pub fn text_antialias(&self) -> AntialiasMode {
        self.canvas.text_antialias()
    }
    pub fn set_fill_paint(&mut self, paint: impl Into<Paint>) {
        self.canvas.set_fill_paint(paint.into());
    }
    pub fn set_stroke_paint(&mut self, paint: impl Into<Paint>) {
        self.canvas.set_stroke_paint(paint.into());
    }
    pub fn stroke_paint(&self) -> Option<&Paint> {
        self.canvas.stroke_paint()
    }
    pub fn set_stroke_style(&mut self, stroke: StrokeStyle) {
        self.canvas.set_stroke(stroke);
    }
    pub fn set_stroke_width(&mut self, width: f64) -> Result<(), GraphicsError> {
        match self.canvas.line_width(width) {
            Ok(()) => Ok(()),
            Err(e) => self.reject(e.into()),
        }
    }
    pub fn set_fill_rule(&mut self, rule: FillRule) {
        self.canvas.set_fill_rule(rule);
    }
    fn validate_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        if [
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            rect.x + rect.width,
            rect.y + rect.height,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return self.reject(
                GraphicsInputError::NonFinite {
                    field: "rectangle",
                    index: None,
                }
                .into(),
            );
        }
        Ok(())
    }
    pub fn set_opacity(&mut self, opacity: f64) -> Result<(), GraphicsError> {
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return self.reject(GraphicsError::InvalidOpacity);
        }
        self.canvas.global_alpha(opacity);
        Ok(())
    }
    pub fn fill_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        self.validate_rect(rect)?;
        self.canvas
            .record_checked(|c| c.fill_rectangle(rect.x, rect.y, rect.width, rect.height))
    }
    pub fn stroke_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        self.validate_rect(rect)?;
        self.canvas
            .record_checked(|c| c.draw_rectangle(rect.x, rect.y, rect.width, rect.height))
    }
    pub fn fill_ellipse(&mut self, bounds: Rectangle) -> Result<(), GraphicsError> {
        self.validate_rect(bounds)?;
        self.canvas.record_checked(|canvas| {
            canvas.fill_oval(bounds.x, bounds.y, bounds.width, bounds.height);
        })
    }
    pub fn stroke_ellipse(&mut self, bounds: Rectangle) -> Result<(), GraphicsError> {
        self.validate_rect(bounds)?;
        self.canvas.record_checked(|canvas| {
            canvas.draw_oval(bounds.x, bounds.y, bounds.width, bounds.height);
        })
    }
    pub fn stroke_line(&mut self, start: Point, end: Point) -> Result<(), GraphicsError> {
        let paint = self.canvas.stroke_paint().cloned();
        let stroke = self.canvas.current_stroke_style().clone();
        self.canvas.record_checked(move |canvas| {
            if let Some(paint) = paint {
                canvas.line_with_style(start, end, paint, stroke);
            }
        })
    }
    pub fn stroke_polyline(&mut self, points: &[Point]) -> Result<(), GraphicsError> {
        let paint = self.canvas.stroke_paint().cloned();
        let stroke = self.canvas.current_stroke_style().clone();
        self.canvas.record_checked(|canvas| {
            if let Some(paint) = paint {
                canvas.polyline_with_style(points, paint, stroke);
            }
        })
    }
    pub fn fill_path(&mut self, path: &Path) -> Result<(), GraphicsError> {
        self.path(path, false)
    }
    pub fn stroke_path(&mut self, path: &Path) -> Result<(), GraphicsError> {
        self.path(path, true)
    }
    fn path(&mut self, path: &Path, stroke: bool) -> Result<(), GraphicsError> {
        if let Err(e) = ClipPath::try_new(path, FillRule::NonZero) {
            return self.reject(e.into());
        }
        self.canvas.record_checked(|c| c.record_path(path, stroke))
    }
    pub fn fill_text(&mut self, layout: &TextLayout, origin: Point) -> Result<(), GraphicsError> {
        self.text(layout, origin, false)
    }
    pub fn fill_styled_text(
        &mut self,
        text: &crate::text::StyledTextLayout,
        origin: Point,
    ) -> Result<(), GraphicsError> {
        self.push_state();
        let result = (|| {
            for (layout, color) in text.paint_runs() {
                self.set_fill_paint(color);
                self.fill_text(layout, origin)?;
            }
            Ok(())
        })();
        let restored = self.pop_state();
        result.and(restored)
    }

    pub fn stroke_text(&mut self, layout: &TextLayout, origin: Point) -> Result<(), GraphicsError> {
        self.text(layout, origin, true)
    }
    fn text(
        &mut self,
        layout: &TextLayout,
        origin: Point,
        stroke: bool,
    ) -> Result<(), GraphicsError> {
        if !origin.x().is_finite() || !origin.y().is_finite() {
            return self.reject(
                GraphicsInputError::NonFinite {
                    field: "text origin",
                    index: None,
                }
                .into(),
            );
        }
        let mut retained = Vec::new();
        if let Some(resources) = self.resources {
            for run in layout.glyph_runs() {
                let resource = match resources.snapshot_resource(run.font.resource_id()) {
                    Ok(r) => r,
                    Err(e) => return self.reject(GraphicsError::Resource(e)),
                };
                if let Err(e) = crate::text::outline::FontInstanceRef::new(
                    &run.font,
                    &resource,
                    &run.normalized_coords,
                ) {
                    return self.reject(GraphicsError::Font(e));
                }
                if self.leases.as_ref().is_some_and(|leases| {
                    leases
                        .iter()
                        .any(|old| old.id == resource.id && old.revision != resource.revision)
                }) {
                    return self.reject(GraphicsError::ConflictingResource(resource.id));
                }
                retained.push(resource);
            }
        }
        self.canvas.record_checked(|c| {
            if stroke {
                c.stroke_text_layout(layout, origin.x(), origin.y());
            } else {
                c.fill_text_layout(layout, origin.x(), origin.y());
            }
        })?;
        if let Some(leases) = self.leases.as_mut() {
            for resource in retained {
                if !leases.iter().any(|old| old.id == resource.id) {
                    leases.push(resource);
                }
            }
        }
        Ok(())
    }

    pub fn push_state(&mut self) {
        self.canvas.push_state();
    }
    pub fn restore_state(&mut self) -> Result<(), GraphicsError> {
        if self.canvas.state_depth() <= self.state_floor {
            return self.reject(GraphicsError::UnbalancedState);
        }
        self.canvas.restore_state();
        Ok(())
    }
    pub fn pop_state(&mut self) -> Result<(), GraphicsError> {
        if self.canvas.state_depth() <= self.state_floor {
            return self.reject(GraphicsError::UnbalancedState);
        }
        self.canvas.pop_state();
        Ok(())
    }
    pub fn concat_transform(&mut self, matrix: crate::Affine2D) -> Result<(), GraphicsError> {
        let [a, b, c, d, e, f] = matrix.coeffs();
        self.canvas
            .record_checked(|gc| gc.transform(a, b, c, d, e, f))
    }
    pub fn rotate_degrees(&mut self, degrees: f64) -> Result<(), GraphicsError> {
        if !degrees.is_finite() {
            return self.reject(
                GraphicsInputError::NonFinite {
                    field: "rotation degrees",
                    index: None,
                }
                .into(),
            );
        }
        self.concat_transform(crate::Affine2D::from_rotation_radians(degrees.to_radians()))
    }
    pub fn set_transform(&mut self, matrix: crate::Affine2D) -> Result<(), GraphicsError> {
        let [a, b, c, d, e, f] = matrix.coeffs();
        self.canvas
            .record_checked(|gc| gc.set_transform(a, b, c, d, e, f))
    }
    pub fn clip_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        self.validate_rect(rect)?;
        self.canvas
            .record_checked(|gc| gc.clip_rect(rect.x, rect.y, rect.width, rect.height))
    }
    pub fn clip_path(&mut self, clip: &ClipPath) -> Result<(), GraphicsError> {
        self.canvas.record_checked(|gc| gc.clip_path(clip))
    }

    /// Replays immutable commands in this context's local domain.
    /// Destination resource identities must already be admitted by Runtime.
    pub fn replay(
        &mut self,
        drawing: &crate::render::RecordedDrawing,
    ) -> Result<(), GraphicsError> {
        if let Some(resources) = self.resources {
            for update in &drawing.resources().ready {
                let actual = resources
                    .snapshot_resource(update.id)
                    .map_err(GraphicsError::Resource)?;
                if actual != *update {
                    return self.reject(GraphicsError::ConflictingResource(update.id));
                }
            }
        }
        if let Some(leases) = &mut self.leases {
            for update in &drawing.resources().ready {
                if leases
                    .iter()
                    .any(|old| old.id == update.id && old != update)
                {
                    return self.reject(GraphicsError::ConflictingResource(update.id));
                }
            }
        }
        self.canvas.replay(drawing)?;
        if let Some(leases) = &mut self.leases {
            for update in &drawing.resources().ready {
                if !leases.iter().any(|old| old.id == update.id) {
                    leases.push(update.clone());
                }
            }
        }
        Ok(())
    }

    /// Draws an exact image resource revision into a logical destination rectangle.
    pub fn draw_image(
        &mut self,
        image: crate::render::ImageResourceRef,
        destination: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.draw_image_region(crate::render::ImageRegion::full(image), destination)
    }

    pub fn draw_image_region(
        &mut self,
        region: crate::render::ImageRegion,
        destination: Rectangle,
    ) -> Result<(), GraphicsError> {
        let image = region.image();
        let source = region.source_pixels();
        match crate::render::command::validate_image_draw_geometry(
            image.width(),
            image.height(),
            source,
            destination,
        ) {
            Ok(crate::render::command::ImageDrawDisposition::Draw) => {}
            Ok(crate::render::command::ImageDrawDisposition::NoOp) => return Ok(()),
            Err(e) => return self.reject(GraphicsError::Image(e)),
        }
        let resource = if let Some(resources) = self.resources {
            let resource = match resources.snapshot_resource(image.resource_id()) {
                Ok(r) => r,
                Err(e) => return self.reject(GraphicsError::Resource(e)),
            };
            if resource.revision != image.revision() {
                return self.reject(GraphicsError::ConflictingResource(resource.id));
            }
            if self.leases.as_ref().is_some_and(|leases| {
                leases
                    .iter()
                    .any(|old| old.id == resource.id && old.revision != resource.revision)
            }) {
                return self.reject(GraphicsError::ConflictingResource(resource.id));
            }
            Some(resource)
        } else {
            None
        };
        self.canvas.record_checked(|gc| {
            // Geometry was checked before entering the atomic recording operation.
            let _ = gc.draw_image_region(region, destination);
        })?;
        if let (Some(resource), Some(leases)) = (resource, self.leases.as_mut())
            && !leases.iter().any(|old| old.id == resource.id)
        {
            leases.push(resource);
        }
        Ok(())
    }
}

impl Drop for PaintContext<'_> {
    fn drop(&mut self) {
        if self.figure_scope && self.canvas.state_depth() != self.state_floor {
            self.canvas.reject_recording(GraphicsError::UnbalancedState);
            while self.canvas.state_depth() > self.state_floor {
                self.canvas.pop_state();
            }
        }
    }
}

/// Unified standalone preparation and drawing facade.
pub struct Graphics<'a> {
    measure: MeasureContext<'a>,
    paint: PaintContext<'a>,
}

impl<'a> Graphics<'a> {
    pub fn new(text: &'a mut TextSystem, recorder: &'a mut crate::render::CommandRecorder) -> Self {
        let (engine, resources) = text.parts();
        Self {
            measure: MeasureContext::new(engine, recorder.canvas.font().clone()),
            paint: PaintContext {
                canvas: &mut recorder.canvas,
                resources: Some(resources),
                leases: Some(&mut recorder.leases),
                state_floor: 0,
                figure_scope: false,
            },
        }
    }
    pub fn font(&self) -> &FontDescriptor {
        self.paint.canvas.font()
    }
    pub fn set_font(&mut self, font: FontDescriptor) {
        self.paint.canvas.set_font(font);
    }
    pub fn font_metrics(&mut self) -> Result<FontMetrics, TextError> {
        self.measure.set_font(self.font().clone());
        self.measure.font_metrics()
    }
    pub fn measure_text(
        &mut self,
        text: &str,
        constraints: TextConstraints,
    ) -> Result<TextMetrics, TextError> {
        self.measure.set_font(self.font().clone());
        self.measure.measure_text(text, constraints)
    }
    pub fn layout_text(
        &mut self,
        text: &str,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        self.measure.set_font(self.font().clone());
        self.measure.layout_text(text, constraints)
    }
    pub fn paint(&mut self) -> PaintContext<'_> {
        PaintContext::for_recorder(
            self.paint.canvas,
            self.paint.resources,
            self.paint.leases.as_deref_mut(),
        )
    }
    pub fn set_antialias(&mut self, mode: AntialiasMode) {
        self.paint.set_antialias(mode);
    }
    pub fn antialias(&self) -> AntialiasMode {
        self.paint.antialias()
    }
    pub fn set_text_antialias(&mut self, mode: AntialiasMode) {
        self.paint.set_text_antialias(mode);
    }
    pub fn text_antialias(&self) -> AntialiasMode {
        self.paint.text_antialias()
    }
    pub fn set_fill_paint(&mut self, paint: impl Into<Paint>) {
        self.paint.set_fill_paint(paint);
    }
    pub fn set_stroke_paint(&mut self, paint: impl Into<Paint>) {
        self.paint.set_stroke_paint(paint);
    }
    pub fn set_stroke_style(&mut self, stroke: StrokeStyle) {
        self.paint.set_stroke_style(stroke);
    }
    pub fn fill_text(&mut self, text: &TextLayout, origin: Point) -> Result<(), GraphicsError> {
        self.paint.fill_text(text, origin)
    }
    pub fn fill_styled_text(
        &mut self,
        text: &crate::text::StyledTextLayout,
        origin: Point,
    ) -> Result<(), GraphicsError> {
        self.paint.fill_styled_text(text, origin)
    }
    pub fn stroke_text(&mut self, text: &TextLayout, origin: Point) -> Result<(), GraphicsError> {
        self.paint.stroke_text(text, origin)
    }
    pub fn fill_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        self.paint.fill_rect(rect)
    }
    pub fn stroke_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        self.paint.stroke_rect(rect)
    }
    pub fn fill_path(&mut self, path: &Path) -> Result<(), GraphicsError> {
        self.paint.fill_path(path)
    }
    pub fn stroke_path(&mut self, path: &Path) -> Result<(), GraphicsError> {
        self.paint.stroke_path(path)
    }
    pub fn push_state(&mut self) {
        self.paint.push_state();
    }
    pub fn restore_state(&mut self) -> Result<(), GraphicsError> {
        self.paint.restore_state()
    }
    pub fn pop_state(&mut self) -> Result<(), GraphicsError> {
        self.paint.pop_state()
    }
    pub fn set_stroke_width(&mut self, width: f64) -> Result<(), GraphicsError> {
        self.paint.set_stroke_width(width)
    }
    pub fn set_fill_rule(&mut self, rule: FillRule) {
        self.paint.set_fill_rule(rule);
    }
    pub fn set_opacity(&mut self, opacity: f64) -> Result<(), GraphicsError> {
        self.paint.set_opacity(opacity)
    }
    pub fn concat_transform(&mut self, matrix: crate::Affine2D) -> Result<(), GraphicsError> {
        self.paint.concat_transform(matrix)
    }
    pub fn rotate_degrees(&mut self, degrees: f64) -> Result<(), GraphicsError> {
        self.paint.rotate_degrees(degrees)
    }
    pub fn set_transform(&mut self, matrix: crate::Affine2D) -> Result<(), GraphicsError> {
        self.paint.set_transform(matrix)
    }
    pub fn clip_rect(&mut self, rect: Rectangle) -> Result<(), GraphicsError> {
        self.paint.clip_rect(rect)
    }
    pub fn clip_path(&mut self, clip: &ClipPath) -> Result<(), GraphicsError> {
        self.paint.clip_path(clip)
    }
    pub fn draw_image(
        &mut self,
        image: crate::render::ImageResourceRef,
        destination: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.paint.draw_image(image, destination)
    }
    pub fn draw_image_region(
        &mut self,
        region: crate::render::ImageRegion,
        destination: Rectangle,
    ) -> Result<(), GraphicsError> {
        self.paint.draw_image_region(region, destination)
    }
}
