//! Unhinted glyph curves in font units, baseline origin, Y upwards.
//! Providers never choose glyphs or alter layout advances.
use super::FontFaceRef;
use crate::graphics::{ClipPath, FillRule, GraphicsInputError, Path, PathOp};
use crate::render::{FontData, ResourcePayload, ResourceUpdate};
use skrifa::{
    FontRef, MetadataProvider,
    instance::{NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FontError {
    ResourceMismatch,
    InvalidFont,
    MissingGlyph(u32),
    UnsupportedFormat,
    InvalidOutline(GraphicsInputError),
    InvalidUnitsPerEm,
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceMismatch => {
                f.write_str("font resource identity or revision does not match")
            }
            Self::InvalidFont => f.write_str("invalid font data or outline"),
            Self::MissingGlyph(id) => write!(f, "glyph {id} is outside the font"),
            Self::UnsupportedFormat => f.write_str("font has no supported monochrome outline"),
            Self::InvalidOutline(e) => e.fmt(f),
            Self::InvalidUnitsPerEm => f.write_str("units per em must be nonzero"),
        }
    }
}
impl std::error::Error for FontError {}

/// One exact face and variation instance, borrowing retained font bytes.
#[derive(Clone, Copy)]
pub struct FontInstanceRef<'a> {
    face: &'a FontFaceRef,
    data: &'a FontData,
    normalized_coords: &'a [i16],
}

impl<'a> FontInstanceRef<'a> {
    pub fn new(
        face: &'a FontFaceRef,
        resource: &'a ResourceUpdate,
        normalized_coords: &'a [i16],
    ) -> Result<Self, FontError> {
        if face.resource_id() != resource.id || face.revision() != resource.revision {
            return Err(FontError::ResourceMismatch);
        }
        let ResourcePayload::Font(data) = &resource.payload else {
            return Err(FontError::ResourceMismatch);
        };
        Ok(Self {
            face,
            data,
            normalized_coords,
        })
    }
    pub fn face(self) -> &'a FontFaceRef {
        self.face
    }
    pub fn bytes(self) -> &'a [u8] {
        self.data.bytes()
    }
    pub fn normalized_coords(self) -> &'a [i16] {
        self.normalized_coords
    }
}

/// Immutable geometry; positions, paint and GPU assets are intentionally separate.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphOutline {
    path: Arc<Path>,
    ink_bounds: Option<crate::Rectangle>,
    units_per_em: u16,
}

impl GlyphOutline {
    pub fn new(path: Path, units_per_em: u16) -> Result<Self, FontError> {
        if units_per_em == 0 {
            return Err(FontError::InvalidUnitsPerEm);
        }
        ClipPath::try_new(&path, FillRule::NonZero).map_err(FontError::InvalidOutline)?;
        for (index, op) in path.operations().iter().enumerate() {
            if matches!(
                op,
                PathOp::Arc { .. } | PathOp::HLineTo(_) | PathOp::VLineTo(_)
            ) {
                return Err(FontError::InvalidOutline(GraphicsInputError::InvalidPath {
                    index,
                }));
            }
        }
        let ink_bounds = path.bounding_box();
        if ink_bounds.is_some_and(|r| {
            [r.x, r.y, r.width, r.height, r.x + r.width, r.y + r.height]
                .iter()
                .any(|v| !v.is_finite())
        }) {
            return Err(FontError::InvalidOutline(GraphicsInputError::NonFinite {
                field: "glyph ink bounds",
                index: None,
            }));
        }
        Ok(Self {
            path: Arc::new(path),
            ink_bounds,
            units_per_em,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn ink_bounds(&self) -> Option<crate::Rectangle> {
        self.ink_bounds
    }
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }
}

mod preparation;
pub use preparation::{
    GlyphGeometryKey, GlyphInstance, OutlineCache, OutlineTextEngine, OutlinedText,
};

pub trait GlyphOutlineProvider {
    /// Advance this revision whenever the provider's geometry policy changes.
    fn revision(&self) -> u64;
    fn outline(&mut self, font: FontInstanceRef<'_>, glyph: u32)
    -> Result<GlyphOutline, FontError>;
}

#[derive(Default)]
pub struct SkrifaOutlineProvider;

impl GlyphOutlineProvider for SkrifaOutlineProvider {
    fn revision(&self) -> u64 {
        0
    }
    fn outline(
        &mut self,
        font: FontInstanceRef<'_>,
        glyph: u32,
    ) -> Result<GlyphOutline, FontError> {
        let face = FontRef::from_index(font.bytes(), font.face.collection_index())
            .map_err(|_| FontError::InvalidFont)?;
        let coords: Vec<_> = font
            .normalized_coords
            .iter()
            .map(|v| NormalizedCoord::from_bits(*v))
            .collect();
        let metrics = face.metrics(Size::unscaled(), coords.as_slice());
        if glyph >= u32::from(metrics.glyph_count) {
            return Err(FontError::MissingGlyph(glyph));
        }
        let outlines = face.outline_glyphs();
        if outlines.format().is_none() {
            return Err(FontError::UnsupportedFormat);
        }
        let outline = outlines
            .get(skrifa::GlyphId::new(glyph))
            .ok_or(FontError::InvalidFont)?;
        let mut pen = PathPen(Path::new());
        outline
            .draw(
                DrawSettings::unhinted(Size::unscaled(), coords.as_slice()),
                &mut pen,
            )
            .map_err(|_| FontError::InvalidFont)?;
        GlyphOutline::new(pen.0, metrics.units_per_em)
    }
}

struct PathPen(Path);
impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x.into(), y.into());
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x.into(), y.into());
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.quad_to(cx.into(), cy.into(), x.into(), y.into());
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        self.0.cubic_to(
            ax.into(),
            ay.into(),
            bx.into(),
            by.into(),
            x.into(),
            y.into(),
        );
    }
    fn close(&mut self) {
        self.0.close();
    }
}
