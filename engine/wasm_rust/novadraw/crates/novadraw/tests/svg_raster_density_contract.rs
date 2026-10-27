use novadraw::{
    geometry::Dimension,
    render::{ImageData, ImageDecodeError, SvgImageSource, SvgRasterOptions},
};

const SVG: &[u8] = br##"<svg width="2" height="3" xmlns="http://www.w3.org/2000/svg"><rect width="2" height="3" fill="#ff0000" fill-opacity="0.5"/></svg>"##;

#[test]
fn retained_source_preserves_natural_size_and_straight_alpha_at_each_density() {
    let source = SvgImageSource::decode(SVG).unwrap();
    assert_eq!(source.logical_size(), Dimension::new(2.0, 3.0));
    for density in [1, 2, 3, 1] {
        let image = source
            .rasterize(SvgRasterOptions::try_new(density, 54).unwrap())
            .unwrap();
        assert_eq!((image.width, image.height), (2 * density, 3 * density));
        assert_eq!(image.scale, f64::from(density));
        assert_eq!(f64::from(image.width) / image.scale, 2.0);
        assert_eq!(f64::from(image.height) / image.scale, 3.0);
        assert_eq!(
            image.pixels.len(),
            (image.width * image.height * 4) as usize
        );
        for pixel in image.pixels.chunks_exact(4) {
            assert_eq!(&pixel[..3], &[255, 0, 0]);
            assert!(matches!(pixel[3], 127 | 128));
        }
    }
}

#[test]
fn invalid_options_dimensions_and_budget_do_not_poison_the_source() {
    assert_eq!(
        SvgRasterOptions::try_new(0, 1),
        Err(ImageDecodeError::InvalidPixelDensity)
    );
    assert_eq!(
        SvgRasterOptions::try_new(1, 0),
        Err(ImageDecodeError::InvalidPixelBudget)
    );
    let source = SvgImageSource::decode(SVG).unwrap();
    assert_eq!(
        source.rasterize(SvgRasterOptions::try_new(u32::MAX, u64::MAX).unwrap()),
        Err(ImageDecodeError::RasterDimensionsOverflow)
    );
    assert_eq!(
        source.rasterize(SvgRasterOptions::try_new(3, 53).unwrap()),
        Err(ImageDecodeError::RasterPixelBudgetExceeded {
            required: 54,
            budget: 53,
        })
    );
    assert!(
        source
            .rasterize(SvgRasterOptions::try_new(1, 6).unwrap())
            .is_ok()
    );
    for invalid in [b"".as_slice(), b"not svg", b"<svg>"] {
        assert!(matches!(
            SvgImageSource::decode(invalid),
            Err(ImageDecodeError::InvalidSvg)
        ));
    }
}

#[test]
fn legacy_scale_remains_metadata_and_keeps_error_precedence() {
    let source = SvgImageSource::decode(SVG).unwrap();
    let raster = source
        .rasterize(SvgRasterOptions::try_new(1, 6).unwrap())
        .unwrap();
    let legacy = ImageData::decode_svg(SVG, 2.5).unwrap();
    assert_eq!((legacy.width, legacy.height, legacy.scale), (2, 3, 2.5));
    assert_eq!(legacy.pixels, raster.pixels);
    assert_eq!(
        ImageData::decode_svg(b"invalid", 0.0),
        Err(ImageDecodeError::InvalidScale)
    );
}

#[test]
fn svg_only_resolves_embedded_images_even_when_an_external_file_exists() {
    let external = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/native/draw2d-official-examples/assets/zoom/class_obj.svg"
    );
    assert!(std::path::Path::new(external).is_file());
    let svg = format!(
        r#"<svg width="16" height="16" xmlns="http://www.w3.org/2000/svg"><image width="16" height="16" href="{external}"/></svg>"#
    );
    let image = SvgImageSource::decode(svg.as_bytes())
        .unwrap()
        .rasterize(SvgRasterOptions::try_new(1, 256).unwrap())
        .unwrap();
    assert!(image.pixels.chunks_exact(4).all(|pixel| pixel[3] == 0));
    assert_eq!(ImageData::decode_svg(svg.as_bytes(), 1.0).unwrap(), image);

    let embedded = br#"<svg width="2" height="3" xmlns="http://www.w3.org/2000/svg"><image width="2" height="3" href="data:image/svg+xml,%3Csvg%20width='2'%20height='3'%20xmlns='http://www.w3.org/2000/svg'%3E%3Crect%20width='2'%20height='3'%20fill='red'/%3E%3C/svg%3E"/></svg>"#;
    let image = SvgImageSource::decode(embedded)
        .unwrap()
        .rasterize(SvgRasterOptions::try_new(1, 6).unwrap())
        .unwrap();
    assert!(image.pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
}
