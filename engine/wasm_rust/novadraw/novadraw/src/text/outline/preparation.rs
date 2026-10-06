use super::*;
use crate::render::{ResourceId, text::GlyphRun};
use crate::text::shaping::TextLayoutEngine;
use crate::text::{FontDescriptor, FontMetrics, TextConstraints, TextError, TextLayout};
use crate::{Affine2D, Point, Rectangle};
use std::collections::HashMap;

/// Stable identity for a font-local asset. Position, size, paint and clip are not part of it.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GlyphGeometryKey {
    provider: uuid::Uuid,
    provider_revision: u64,
    face: FontFaceRef,
    glyph: u32,
    coords: Vec<i16>,
}

impl GlyphGeometryKey {
    pub fn face(&self) -> &FontFaceRef {
        &self.face
    }
    pub fn glyph(&self) -> u32 {
        self.glyph
    }
    pub fn normalized_coords(&self) -> &[i16] {
        &self.coords
    }
    pub fn provider_revision(&self) -> u64 {
        self.provider_revision
    }
}

/// Layout placement separate from the shared font-local curve asset.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphInstance {
    key: GlyphGeometryKey,
    outline: Arc<GlyphOutline>,
    transform: Affine2D,
}

impl GlyphInstance {
    pub fn key(&self) -> &GlyphGeometryKey {
        &self.key
    }
    pub fn outline(&self) -> &Arc<GlyphOutline> {
        &self.outline
    }
    /// Font units, Y up → layout coordinates, Y down. Advance/offset is already included.
    pub fn glyph_to_layout(&self) -> Affine2D {
        self.transform
    }
}

/// Neutral CPU geometry snapshot. Backend-specific preprocessed assets remain outside this value.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlinedText {
    instances: Vec<GlyphInstance>,
    path: Path,
    ink_bounds: Option<Rectangle>,
}

impl OutlinedText {
    pub fn instances(&self) -> &[GlyphInstance] {
        &self.instances
    }
    pub fn ink_bounds(&self) -> Option<Rectangle> {
        self.ink_bounds
    }
    /// Curves in layout coordinates; paint remains in the Graphics logical coordinate domain.
    pub fn path_at(&self, origin: Point) -> Result<Path, FontError> {
        let mut path = Path::new();
        append_transformed(
            &mut path,
            &self.path,
            Affine2D::from_translation(origin.x(), origin.y()),
        )?;
        Ok(path)
    }
}

/// Explicit, replaceable CPU outline service. Dropping the cache does not invalidate snapshots.
pub struct OutlineCache {
    provider: Box<dyn GlyphOutlineProvider>,
    identity: uuid::Uuid,
    revision: u64,
    outlines: HashMap<GlyphGeometryKey, Arc<GlyphOutline>>,
}

impl Default for OutlineCache {
    fn default() -> Self {
        Self::new(Box::new(SkrifaOutlineProvider))
    }
}

impl OutlineCache {
    pub fn new(provider: Box<dyn GlyphOutlineProvider>) -> Self {
        Self {
            revision: provider.revision(),
            provider,
            identity: uuid::Uuid::new_v4(),
            outlines: HashMap::new(),
        }
    }
    pub fn replace_provider(&mut self, provider: Box<dyn GlyphOutlineProvider>) {
        *self = Self::new(provider);
    }
    pub fn clear(&mut self) {
        self.outlines.clear();
    }
    pub fn len(&self) -> usize {
        self.outlines.len()
    }
    pub fn is_empty(&self) -> bool {
        self.outlines.is_empty()
    }

    pub fn prepare(
        &mut self,
        layout: &TextLayout,
        resources: &crate::runtime::ResourceRegistry,
    ) -> Result<OutlinedText, FontError> {
        self.prepare_with(layout, |id| {
            resources
                .snapshot_resource(id)
                .map_err(|_| FontError::ResourceMismatch)
        })
    }

    fn prepare_with(
        &mut self,
        layout: &TextLayout,
        mut resource: impl FnMut(ResourceId) -> Result<ResourceUpdate, FontError>,
    ) -> Result<OutlinedText, FontError> {
        let revision = self.provider.revision();
        if revision != self.revision {
            self.outlines.clear();
            self.revision = revision;
        }
        // Resolve all resources before invoking the provider.
        let fonts = layout
            .glyph_runs()
            .iter()
            .map(|run| {
                let data = resource(run.font.resource_id())?;
                FontInstanceRef::new(&run.font, &data, &run.normalized_coords)?;
                Ok(data)
            })
            .collect::<Result<Vec<_>, FontError>>()?;
        let mut instances = Vec::new();
        let mut path = Path::new();
        for (run, data) in layout.glyph_runs().iter().zip(&fonts) {
            let font = FontInstanceRef::new(&run.font, data, &run.normalized_coords)?;
            for glyph in &run.glyphs {
                let key = GlyphGeometryKey {
                    provider: self.identity,
                    provider_revision: revision,
                    face: run.font.clone(),
                    glyph: glyph.id,
                    coords: run.normalized_coords.clone(),
                };
                let outline = match self.outlines.get(&key) {
                    Some(outline) => outline.clone(),
                    None => {
                        let outline = Arc::new(self.provider.outline(font, glyph.id)?);
                        self.outlines.insert(key.clone(), outline.clone());
                        outline
                    }
                };
                let transform = glyph_transform(run, glyph.x, glyph.y, outline.units_per_em());
                append_transformed(&mut path, outline.path(), transform)?;
                instances.push(GlyphInstance {
                    key,
                    outline,
                    transform,
                });
            }
        }
        // Revalidate derived curves/bounds, including overflow caused by instance transforms.
        let checked = GlyphOutline::new(path.clone(), 1)?;
        Ok(OutlinedText {
            instances,
            ink_bounds: checked.ink_bounds(),
            path,
        })
    }
}

fn glyph_transform(run: &GlyphRun, x: f32, y: f32, units_per_em: u16) -> Affine2D {
    let scale = f64::from(run.font_size) / f64::from(units_per_em);
    let skew = run
        .skew_degrees
        .map_or(0.0, |degrees| f64::from(degrees.to_radians().tan()));
    Affine2D::new(scale, 0.0, scale * skew, -scale, f64::from(x), f64::from(y))
}

fn append_transformed(
    path: &mut Path,
    source: &Path,
    transform: Affine2D,
) -> Result<(), FontError> {
    for (index, op) in source.operations().iter().enumerate() {
        let point = |p: Point| -> Result<Point, FontError> {
            let p = transform.transform_point(p);
            if !p.x().is_finite() || !p.y().is_finite() {
                return Err(FontError::InvalidOutline(GraphicsInputError::NonFinite {
                    field: "glyph transform",
                    index: Some(index),
                }));
            }
            Ok(p)
        };
        match *op {
            PathOp::MoveTo(p) => {
                let p = point(p)?;
                path.move_to(p.x(), p.y());
            }
            PathOp::LineTo(p) => {
                let p = point(p)?;
                path.line_to(p.x(), p.y());
            }
            PathOp::QuadTo(c, p) => {
                let c = point(c)?;
                let p = point(p)?;
                path.quad_to(c.x(), c.y(), p.x(), p.y());
            }
            PathOp::CubicTo(a, b, p) => {
                let a = point(a)?;
                let b = point(b)?;
                let p = point(p)?;
                path.cubic_to(a.x(), a.y(), b.x(), b.y(), p.x(), p.y());
            }
            PathOp::Close => path.close(),
            _ => {
                return Err(FontError::InvalidOutline(GraphicsInputError::InvalidPath {
                    index,
                }));
            }
        }
    }
    Ok(())
}

/// Composition adapter: keeps shaping unchanged, then prepares the selected outline provider.
///
/// Install at `Runtime::with_text_layout_engine` or `TextSystem::with_engine` before registering
/// fonts. Font bytes retain the same Registry identity/revision; no second font collection is
/// created. Default runtimes keep the native glyph path without this adapter.
pub struct OutlineTextEngine {
    engine: Box<dyn TextLayoutEngine>,
    cache: OutlineCache,
    fonts: HashMap<ResourceId, ResourceUpdate>,
    revision: std::cell::Cell<(u64, u64, u64)>,
}

impl OutlineTextEngine {
    pub fn new(engine: Box<dyn TextLayoutEngine>, provider: Box<dyn GlyphOutlineProvider>) -> Self {
        let observed = (engine.revision(), provider.revision(), 0);
        Self {
            engine,
            cache: OutlineCache::new(provider),
            fonts: HashMap::new(),
            revision: std::cell::Cell::new(observed),
        }
    }

    pub fn replace_provider(&mut self, provider: Box<dyn GlyphOutlineProvider>) {
        self.cache.replace_provider(provider);
        let (_, _, revision) = self.revision.get();
        self.revision.set((
            self.engine.revision(),
            self.cache.provider.revision(),
            revision.wrapping_add(1),
        ));
    }

    pub fn cached_outline_count(&self) -> usize {
        self.cache.len()
    }
}

impl TextLayoutEngine for OutlineTextEngine {
    fn revision(&self) -> u64 {
        let (engine, provider, revision) = self.revision.get();
        let next = (self.engine.revision(), self.cache.provider.revision());
        if (engine, provider) != next {
            self.revision
                .set((next.0, next.1, revision.wrapping_add(1)));
        }
        self.revision.get().2
    }
    fn font_metrics(&mut self, font: &FontDescriptor) -> Result<FontMetrics, TextError> {
        self.engine.font_metrics(font)
    }
    fn register_font(
        &mut self,
        id: ResourceId,
        revision: u64,
        bytes: &[u8],
    ) -> Result<(), TextError> {
        self.engine.register_font(id, revision, bytes)?;
        self.fonts.insert(
            id,
            ResourceUpdate {
                id,
                revision,
                payload: ResourcePayload::Font(Arc::new(FontData::new(bytes.to_vec()))),
            },
        );
        self.cache
            .outlines
            .retain(|key, _| key.face.resource_id() != id);
        Ok(())
    }
    fn remove_font(&mut self, id: ResourceId) {
        self.engine.remove_font(id);
        self.fonts.remove(&id);
        self.cache
            .outlines
            .retain(|key, _| key.face.resource_id() != id);
    }
    fn layout(
        &mut self,
        text: &str,
        font: &FontDescriptor,
        constraints: TextConstraints,
    ) -> Result<TextLayout, TextError> {
        let mut layout = self.engine.layout(text, font, constraints)?;
        let outlines = self
            .cache
            .prepare_with(&layout, |id| {
                self.fonts
                    .get(&id)
                    .cloned()
                    .ok_or(FontError::ResourceMismatch)
            })
            .map_err(TextError::Outline)?;
        layout.outlines = Some(Arc::new(outlines));
        layout.set_engine_revision(self.revision());
        Ok(layout)
    }
}
