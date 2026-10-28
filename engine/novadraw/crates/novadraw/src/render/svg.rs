//! Retained SVG decoding and explicitly bounded rasterization.

use crate::geometry::Dimension;

use super::{ImageData, ImageDecodeError};

/// Positive integer pixel density and a caller-supplied allocation budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgRasterOptions {
    pixel_density: u32,
    pixel_budget: u64,
}

impl SvgRasterOptions {
    /// Rejects zero density or budget before rasterization.
    pub fn try_new(pixel_density: u32, pixel_budget: u64) -> Result<Self, ImageDecodeError> {
        if pixel_density == 0 {
            return Err(ImageDecodeError::InvalidPixelDensity);
        }
        if pixel_budget == 0 {
            return Err(ImageDecodeError::InvalidPixelBudget);
        }
        Ok(Self {
            pixel_density,
            pixel_budget,
        })
    }

    pub const fn pixel_density(self) -> u32 {
        self.pixel_density
    }

    pub const fn pixel_budget(self) -> u64 {
        self.pixel_budget
    }
}

/// Immutable, owned SVG source without resource or host state.
pub struct SvgImageSource {
    tree: resvg::usvg::Tree,
}

impl std::fmt::Debug for SvgImageSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SvgImageSource")
            .field("logical_size", &self.logical_size())
            .finish_non_exhaustive()
    }
}

impl SvgImageSource {
    /// Parses SVG with built-in fonts and embedded images, without reading external paths.
    pub fn decode(bytes: &[u8]) -> Result<Self, ImageDecodeError> {
        let mut options = resvg::usvg::Options::default();
        // usvg's default string resolver reads local files. Core decoders only
        // consume supplied bytes; external images must be embedded by the host.
        options.image_href_resolver.resolve_string = Box::new(|_, _| None);
        let fontdb = options.fontdb_mut();
        for font in super::text::BuiltinFont::ALL {
            fontdb.load_font_data(font.bytes().to_vec());
        }
        fontdb.set_sans_serif_family(super::text::BuiltinFont::Inter.family());
        fontdb.set_serif_family(super::text::BuiltinFont::Inter.family());
        fontdb.set_monospace_family(super::text::BuiltinFont::JetBrainsMono.family());
        let tree = resvg::usvg::Tree::from_data(bytes, &options)
            .map_err(|_| ImageDecodeError::InvalidSvg)?;
        Ok(Self { tree })
    }

    /// Returns the intrinsic integer size used by the legacy SVG decoder.
    pub fn logical_size(&self) -> Dimension {
        let size = self.tree.size().to_int_size();
        Dimension::new(f64::from(size.width()), f64::from(size.height()))
    }

    /// Checks dimensions and budget before allocation, preserving natural size.
    pub fn rasterize(&self, options: SvgRasterOptions) -> Result<ImageData, ImageDecodeError> {
        let size = self.tree.size().to_int_size();
        let width = size
            .width()
            .checked_mul(options.pixel_density)
            .ok_or(ImageDecodeError::RasterDimensionsOverflow)?;
        let height = size
            .height()
            .checked_mul(options.pixel_density)
            .ok_or(ImageDecodeError::RasterDimensionsOverflow)?;
        let pixels = u64::from(width) * u64::from(height);
        if pixels > options.pixel_budget {
            return Err(ImageDecodeError::RasterPixelBudgetExceeded {
                required: pixels,
                budget: options.pixel_budget,
            });
        }
        // Validate RGBA addressability too, including on 32-bit Wasm.
        pixels
            .checked_mul(4)
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or(ImageDecodeError::RasterDimensionsOverflow)?;
        self.rasterize_pixels(width, height, options.pixel_density)
    }

    pub(super) fn rasterize_intrinsic(&self) -> Result<ImageData, ImageDecodeError> {
        let size = self.tree.size().to_int_size();
        self.rasterize_pixels(size.width(), size.height(), 1)
    }

    fn rasterize_pixels(
        &self,
        width: u32,
        height: u32,
        density: u32,
    ) -> Result<ImageData, ImageDecodeError> {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or(ImageDecodeError::RasterizationFailed)?;
        resvg::render(
            &self.tree,
            resvg::tiny_skia::Transform::from_scale(density as f32, density as f32),
            &mut pixmap.as_mut(),
        );
        let mut pixels = pixmap.take();
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            if alpha == 0 || alpha == 255 {
                continue;
            }
            for component in &mut pixel[..3] {
                *component = ((u32::from(*component) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
        Ok(ImageData::from_rgba(
            width,
            height,
            pixels,
            f64::from(density),
        ))
    }
}
