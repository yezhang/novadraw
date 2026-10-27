use novadraw::{
    render::{BuiltinFont, ImageData, ImageDecodeError, ResourceId},
    text::{
        FontDecorationMetrics, FontDescriptor, FontMetrics, TextError,
        shaping::{ParleyTextEngine, TextLayoutEngine},
    },
};

#[test]
fn underline_metrics_scale_with_the_resolved_face_and_external_engines_share_the_contract() {
    let mut engine = ParleyTextEngine::new();
    engine
        .register_font(
            ResourceId::new(uuid::Uuid::nil(), 1),
            1,
            BuiltinFont::Inter.bytes(),
        )
        .unwrap();
    let small = engine
        .font_metrics(&FontDescriptor::new(BuiltinFont::Inter.family(), 12.0).unwrap())
        .unwrap();
    let large = engine
        .font_metrics(&FontDescriptor::new(BuiltinFont::Inter.family(), 24.0).unwrap())
        .unwrap();
    let a = small.underline().unwrap();
    let b = large.underline().unwrap();
    assert!(a.offset() > 0.0 && a.thickness() > 0.0);
    assert!((b.offset() - a.offset() * 2.0).abs() < 1e-5);
    assert!((b.thickness() - a.thickness() * 2.0).abs() < 1e-5);
    let external = FontMetrics::new(
        small.face().clone(),
        small.size(),
        vec![],
        small.ascent(),
        small.descent(),
        small.leading(),
    )
    .unwrap();
    assert_eq!(external.underline(), Err(TextError::FontMetricsUnsupported));
    assert_eq!(external.with_underline(a).underline(), Ok(a));
    for (offset, thickness) in [
        (f32::NAN, 1.0),
        (0.0, 0.0),
        (0.0, -1.0),
        (0.0, f32::INFINITY),
    ] {
        assert_eq!(
            FontDecorationMetrics::new(offset, thickness),
            Err(TextError::InvalidMetric)
        );
    }
}

#[test]
fn original_ico_decodes_without_conversion_and_invalid_inputs_fail_explicitly() {
    let bytes =
        include_bytes!("../../../examples/native/draw2d-official-examples/assets/brazil.ico");
    let image = ImageData::decode(bytes, 1.0).unwrap();
    assert!(image.width > 0 && image.height > 0);
    assert_eq!(
        image.pixels.len(),
        image.width as usize * image.height as usize * 4
    );
    assert_eq!(
        ImageData::decode(&bytes[..4], 1.0),
        Err(ImageDecodeError::InvalidIco)
    );
    assert_eq!(
        ImageData::decode(bytes, 0.0),
        Err(ImageDecodeError::InvalidScale)
    );
}
