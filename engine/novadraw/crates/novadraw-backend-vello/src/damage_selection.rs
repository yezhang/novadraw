//! Backend-private ink envelopes, evaluated before Vello decodes bitmap glyphs.
use novadraw::render::{GlyphPaint, GlyphRun, PositionedGlyph};
use skrifa::{GlyphId, MetadataProvider, instance::Size, raw::TableProvider};
use std::collections::HashMap;
use vello::kurbo::{Affine, Rect, Shape};

const MAX_ENTRIES: usize = 65_536;
const MAX_COORDS: usize = 64;
const COVERAGE_PAD: f64 = 2.0;
// Match Vello 0.11's Sbix placement, rather than introducing an Emoji fast path.
const APPLE_BASELINE_DIVISOR: f64 = 8.0;

#[derive(Clone, Copy)]
enum Ink {
    Unknown,
    Empty,
    Bounds(Rect),
}

#[derive(Hash, PartialEq, Eq)]
struct Key {
    face: (novadraw::render::ResourceId, u64, u32),
    size: u32,
    skew: Option<u32>,
    coords: std::sync::Arc<[i16]>,
    glyph: u32,
}

#[derive(Default)]
pub(super) struct Cache(HashMap<Key, Ink>);

impl Cache {
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    pub(super) fn remove(&mut self, id: novadraw::render::ResourceId) {
        self.0.retain(|key, _| key.face.0 != id);
    }

    pub(super) fn select(
        &mut self,
        font: &vello::peniko::FontData,
        run: &GlyphRun,
        placement: (novadraw::geometry::Point, Affine, f64),
        paint: &GlyphPaint,
        clip: Rect,
    ) -> Vec<PositionedGlyph> {
        let (origin, transform, scale) = placement;
        run.glyphs
            .iter()
            .copied()
            .filter(|glyph| {
                // Strokes and unsupported variation dimensions remain conservative.
                if !matches!(paint, GlyphPaint::Fill(_)) || run.normalized_coords.len() > MAX_COORDS
                {
                    return true;
                }
                let key = Key {
                    face: (run.font.resource_id(), run.font.revision(), font.index),
                    size: (run.font_size * scale as f32).to_bits(),
                    skew: run.skew_degrees.map(f32::to_bits),
                    coords: run.normalized_coords.clone(),
                    glyph: glyph.id,
                };
                let ink = if let Some(ink) = self.0.get(&key) {
                    *ink
                } else {
                    let ink = ink(font, run, glyph.id, scale).unwrap_or(Ink::Unknown);
                    if self.0.len() < MAX_ENTRIES {
                        self.0.insert(key, ink);
                    }
                    ink
                };
                match ink {
                    Ink::Unknown => true,
                    Ink::Empty => false,
                    Ink::Bounds(bounds) => {
                        let offset = (
                            f64::from((origin.x() as f32 + glyph.x) * scale as f32),
                            f64::from((origin.y() as f32 + glyph.y) * scale as f32),
                        );
                        visible(
                            (transform * Affine::translate(offset)).transform_rect_bbox(bounds),
                            clip,
                        )
                    }
                }
            })
            .collect()
    }
}

pub(super) fn visible(bounds: Rect, clip: Rect) -> bool {
    if !bounds.is_finite() || !clip.is_finite() {
        return true;
    }
    let bounds = bounds.inflate(COVERAGE_PAD, COVERAGE_PAD);
    bounds.x0 < clip.x1 && bounds.x1 > clip.x0 && bounds.y0 < clip.y1 && bounds.y1 > clip.y0
}

fn ink(data: &vello::peniko::FontData, run: &GlyphRun, id: u32, scale: f64) -> Option<Ink> {
    let font = skrifa::FontRef::from_index(data.data.as_ref(), data.index).ok()?;
    let id = GlyphId::new(id);
    // COLR painting may use multiple glyphs, transforms and arbitrary brushes.
    if font.color_glyphs().get(id).is_some() {
        return Some(Ink::Unknown);
    }
    let size = run.font_size * scale as f32;
    let skew = run
        .skew_degrees
        .map(|degrees| Affine::skew(f64::from(degrees.to_radians().tan()), 0.0))
        .unwrap_or(Affine::IDENTITY);
    let strikes = font.bitmap_strikes();
    if let Some(bitmap) = strikes.glyph_for_size(Size::new(size), id) {
        let upem = f32::from(font.head().ok()?.units_per_em());
        let apple = strikes.format() == Some(skrifa::bitmap::BitmapFormat::Sbix)
            && font
                .localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME)
                .any(|name| name.chars().eq("AppleColorEmoji".chars()));
        let outer = if apple {
            (0.0, f64::from(size) / APPLE_BASELINE_DIVISOR)
        } else {
            (
                f64::from(bitmap.bearing_x * (size / upem)),
                -f64::from(bitmap.bearing_y * (size / upem)),
            )
        };
        let bottom = if matches!(bitmap.placement_origin, skrifa::bitmap::Origin::BottomLeft) {
            -f64::from(bitmap.height)
        } else {
            0.0
        };
        let transform = Affine::translate(outer)
            * Affine::scale(f64::from(size / bitmap.ppem_y))
            * Affine::translate((
                f64::from(bitmap.inner_bearing_x),
                -f64::from(bitmap.inner_bearing_y),
            ))
            * Affine::translate((0.0, bottom))
            * skew;
        return Some(Ink::Bounds(transform.transform_rect_bbox(Rect::new(
            0.0,
            0.0,
            f64::from(bitmap.width),
            f64::from(bitmap.height),
        ))));
    }
    let path = super::glyph_outline::outline(data, run, id.to_u32(), scale)?;
    if path.elements().is_empty() {
        return Some(Ink::Empty);
    }
    // Outline coordinates are Y-up; Vello applies glyph skew before this flip.
    let bounds = (Affine::scale_non_uniform(1.0, -1.0) * skew * path).bounding_box();
    Some(Ink::Bounds(bounds))
}
