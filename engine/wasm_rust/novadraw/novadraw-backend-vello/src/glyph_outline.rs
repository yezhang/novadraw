//! CPU outlines for dashed glyph strokes (Vello's glyph cache ignores dashes).

use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{NormalizedCoord, Size},
};
use vello::kurbo::BezPath;

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
