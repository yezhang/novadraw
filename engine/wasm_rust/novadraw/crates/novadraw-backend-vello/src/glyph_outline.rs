//! CPU outlines for dashed glyph strokes (Vello's glyph cache ignores dashes).

use super::antialias::{AntialiasConfig, Work, limit_failure, reserve};
use novadraw::render::{GraphicsInputError, InvalidGraphicsInput, RenderFailure};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{NormalizedCoord, Size},
};
use vello::kurbo::BezPath;
use vello::kurbo::{PathEl, Point};

struct PathPen(BezPath);

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0
            .quad_to((f64::from(cx), f64::from(cy)), (f64::from(x), f64::from(y)));
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        self.0.curve_to(
            (f64::from(ax), f64::from(ay)),
            (f64::from(bx), f64::from(by)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

pub(super) fn outline(
    font: &vello::peniko::FontData,
    run: &novadraw::render::GlyphRun,
    glyph_id: u32,
    scale: f64,
) -> Option<BezPath> {
    let font = skrifa::FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let outlines = font.outline_glyphs();
    let outline = outlines.get(GlyphId::new(glyph_id))?;
    let coords: Vec<_> = run
        .normalized_coords
        .iter()
        .map(|coord| NormalizedCoord::from_bits(*coord))
        .collect();
    let settings =
        DrawSettings::unhinted(Size::new(run.font_size * scale as f32), coords.as_slice());
    let mut pen = PathPen(BezPath::new());
    outline.draw(settings, &mut pen).ok()?;
    Some(pen.0)
}

struct BoundedPen {
    elements: Vec<PathEl>,
    work: Work,
    config: AntialiasConfig,
    index: usize,
    failure: Option<RenderFailure>,
}

impl BoundedPen {
    fn push(&mut self, element: PathEl) {
        if self.failure.is_some() {
            return;
        }
        let result = self
            .work
            .add(
                Work {
                    elements: 1,
                    ..Work::default()
                },
                self.config.operation,
                self.index,
            )
            .and_then(|()| reserve(&mut self.elements, 1));
        match result {
            Ok(()) => self.elements.push(element),
            Err(error) => self.failure = Some(error),
        }
    }
}

impl OutlinePen for BoundedPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.push(PathEl::MoveTo(Point::new(x.into(), y.into())));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(PathEl::LineTo(Point::new(x.into(), y.into())));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.push(PathEl::QuadTo(
            Point::new(cx.into(), cy.into()),
            Point::new(x.into(), y.into()),
        ));
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        self.push(PathEl::CurveTo(
            Point::new(ax.into(), ay.into()),
            Point::new(bx.into(), by.into()),
            Point::new(x.into(), y.into()),
        ));
    }
    fn close(&mut self) {
        self.push(PathEl::ClosePath);
    }
}

pub(super) fn bounded_outline(
    data: &vello::peniko::FontData,
    run: &novadraw::render::GlyphRun,
    glyph_id: u32,
    config: AntialiasConfig,
    command_index: usize,
) -> Result<(BezPath, Work), RenderFailure> {
    let unavailable = || {
        RenderFailure::InvalidInput(InvalidGraphicsInput {
            command_index,
            reason: GraphicsInputError::AntialiasOutlineUnavailable { glyph_id },
        })
    };
    let font =
        skrifa::FontRef::from_index(data.data.as_ref(), data.index).map_err(|_| unavailable())?;
    let outlines = font.outline_glyphs();
    let outline = outlines
        .get(GlyphId::new(glyph_id))
        .ok_or_else(unavailable)?;
    let scratch_size = outline.draw_memory_size(skrifa::outline::Hinting::None);
    let coords_bytes = run
        .normalized_coords
        .len()
        .saturating_mul(std::mem::size_of::<NormalizedCoord>());
    let requested = scratch_size.saturating_add(coords_bytes);
    if requested > config.glyph_scratch_bytes {
        return Err(limit_failure(
            command_index,
            "glyph scratch bytes",
            requested,
            config.glyph_scratch_bytes,
        ));
    }
    let mut scratch = Vec::new();
    reserve(&mut scratch, scratch_size)?;
    scratch.resize(scratch_size, 0_u8);
    let mut coords = Vec::new();
    reserve(&mut coords, run.normalized_coords.len())?;
    coords.extend(
        run.normalized_coords
            .iter()
            .map(|value| NormalizedCoord::from_bits(*value)),
    );
    let settings = DrawSettings::unhinted(Size::new(run.font_size), coords.as_slice())
        .with_memory(Some(scratch.as_mut_slice()));
    let mut pen = BoundedPen {
        elements: Vec::new(),
        work: Work::default(),
        config,
        index: command_index,
        failure: None,
    };
    outline
        .draw(settings, &mut pen)
        .map_err(|_| unavailable())?;
    if let Some(error) = pen.failure {
        return Err(error);
    }
    Ok((BezPath::from_vec(pen.elements), pen.work))
}
