use novadraw::{
    animation::{AnimationError, AnimationValue},
    figure::{Figure, FigureStyle},
    geometry::Rectangle,
    graphics::{GraphicsInputError, Opacity, PaintContext},
    render::{CommandRecorder, RenderCommandKind},
    runtime::Runtime,
    text::{FontDescriptor, FontStyle},
};
use std::error::Error;

#[test]
fn opacity_preserves_endpoints_and_classifies_input_errors() {
    for scalar in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = Opacity::try_new(scalar).unwrap_err();
        assert_eq!(
            error,
            GraphicsInputError::NonFinite {
                field: "opacity",
                index: None
            }
        );
        let wrapped = AnimationError::from(error);
        assert_eq!(
            wrapped
                .source()
                .unwrap()
                .downcast_ref::<GraphicsInputError>(),
            Some(&error)
        );
    }
    for scalar in [-0.01, 1.01] {
        assert_eq!(
            Opacity::try_new(scalar),
            Err(GraphicsInputError::OpacityOutOfRange)
        );
    }
    for scalar in [0.0, 0.5, 1.0] {
        assert_eq!(Opacity::try_new(scalar).unwrap().get(), scalar);
    }
    assert_eq!(std::mem::size_of::<Opacity>(), std::mem::size_of::<f64>());
    assert_eq!(
        Opacity::TRANSPARENT.interpolate(&Opacity::OPAQUE, 2.0),
        Opacity::OPAQUE
    );
    assert_eq!(
        Opacity::OPAQUE.interpolate(&Opacity::TRANSPARENT, 2.0),
        Opacity::TRANSPARENT
    );
}

#[test]
fn font_tokens_preserve_numeric_weight_signs_and_unicode() {
    for (text, weight, style, family) in [
        (
            "700 12px Inter Variable",
            700.0,
            FontStyle::Normal,
            "Inter Variable",
        ),
        (
            "italic 1 12px 未安装字体 名称",
            1.0,
            FontStyle::Italic,
            "未安装字体 名称",
        ),
        (
            "1000 oblique 12px Inter",
            1000.0,
            FontStyle::Oblique,
            "Inter",
        ),
        (
            "normal 12px sans-serif",
            400.0,
            FontStyle::Normal,
            "sans-serif",
        ),
        ("bold italic 12px Inter", 700.0, FontStyle::Italic, "Inter"),
        (" 12px\u{2003}字体 ", 400.0, FontStyle::Normal, "字体"),
    ] {
        let descriptor = FontDescriptor::parse(text).unwrap();
        assert_eq!(descriptor.size(), 12.0);
        assert_eq!(descriptor.weight(), weight);
        assert_eq!(descriptor.style(), style);
        assert_eq!(descriptor.family(), family);
        assert_eq!(
            FontDescriptor::parse(&descriptor.to_string()).unwrap(),
            descriptor
        );
    }
    for text in [
        "0px Inter",
        "-12px Inter",
        "NaNpx Inter",
        "infpx Inter",
        "12px",
        "700 700 12px Inter",
        "italic oblique 12px Inter",
        "normal bold 12px Inter",
        "unknown 12px Inter",
        "0 12px Inter",
        "1001 12px Inter",
        "12pxInter",
        "你好12px Inter",
        "bold bold 12px Inter",
    ] {
        assert!(FontDescriptor::parse(text).is_err(), "{text}");
    }
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(FontDescriptor::new("Inter", size).is_err());
    }
    assert!(FontDescriptor::new(" \u{2003}", 12.0).is_err());
    for weight in [0.0, 1001.0, f32::NAN, f32::INFINITY] {
        assert!(FontDescriptor::default().with_weight(weight).is_err());
    }
}

struct ExternalStyledFigure(FigureStyle);
impl novadraw::figure::FigureContainer for ExternalStyledFigure {}
impl Figure for ExternalStyledFigure {
    fn name(&self) -> &'static str {
        "ExternalStyledFigure"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(-1.0, 0.0, 40.0, 30.0)
    }
    fn initial_style(&self) -> FigureStyle {
        self.0.clone()
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::figure::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::figure::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::CONTAINER,
            novadraw::figure::ContainerCapability::of::<Self>(),
        )
    }
    fn paint(
        &self,
        context: &mut PaintContext<'_>,
        bounds: Rectangle,
    ) -> Result<(), novadraw::graphics::GraphicsError> {
        context.fill_rect(bounds)
    }
}

#[test]
fn external_styles_inherit_override_clear_and_report_same_value_unchanged() {
    let parent_font = FontDescriptor::parse("700 12px Inter Variable").unwrap();
    let child_font = FontDescriptor::parse("italic 18px 未安装字体").unwrap();
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(ExternalStyledFigure(FigureStyle {
            alpha: Some(Opacity::try_new(0.5).unwrap()),
            font: Some(parent_font.clone()),
            ..Default::default()
        })))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(ExternalStyledFigure(FigureStyle::default())))
        .unwrap();
    let inherited = runtime.tree().resolved_style(child).unwrap();
    assert_eq!(inherited.alpha.get(), 0.5);
    assert_eq!(inherited.font, parent_font);
    let override_style = FigureStyle {
        alpha: Some(Opacity::try_new(0.8).unwrap()),
        font: Some(child_font.clone()),
        ..Default::default()
    };
    assert!(
        runtime
            .figure(child)
            .unwrap()
            .set_style(override_style.clone())
            .unwrap()
    );
    let resolved = runtime.tree().resolved_style(child).unwrap();
    assert_eq!(resolved.alpha.get(), 0.8);
    assert_eq!(resolved.font, child_font);
    assert!(
        !runtime
            .figure(child)
            .unwrap()
            .set_style(override_style)
            .unwrap()
    );
    assert!(Opacity::try_new(f64::NAN).is_err());
    assert_eq!(runtime.tree().resolved_style(child).unwrap(), resolved);
    runtime
        .figure(child)
        .unwrap()
        .set_style(FigureStyle::default())
        .unwrap();
    assert_eq!(runtime.tree().resolved_style(child).unwrap(), inherited);
}

#[test]
fn public_graphics_installs_checked_opacity_without_saturation() {
    let mut recorder = CommandRecorder::new();
    recorder
        .paint()
        .set_opacity(Opacity::try_new(0.5).unwrap())
        .unwrap();
    assert!(matches!(
        recorder.commands()[0].kind,
        RenderCommandKind::SetGlobalAlpha { alpha: 0.5 }
    ));
}

#[test]
fn unknown_family_can_measure_and_draw_with_the_same_descriptor() {
    use novadraw::{
        geometry::Point,
        graphics::Graphics,
        text::{BuiltinFont, TextConstraints, TextSystem},
    };
    let descriptor = FontDescriptor::parse("700 12px 未安装字体 名称").unwrap();
    let mut text = TextSystem::new();
    text.register_builtin_font(BuiltinFont::Inter).unwrap();
    let mut recorder = CommandRecorder::new();
    let mut graphics = Graphics::new(&mut text, &mut recorder);
    graphics.set_font(descriptor.clone());
    graphics
        .paint()
        .set_fill_paint(novadraw::graphics::Color::BLACK);
    let layout = graphics
        .layout_text("office e\u{301}", TextConstraints::UNBOUNDED)
        .unwrap();
    assert_eq!(
        graphics
            .measure_text("office e\u{301}", TextConstraints::UNBOUNDED)
            .unwrap(),
        layout.metrics()
    );
    graphics.paint().fill_text(&layout, Point::ZERO).unwrap();
    drop(graphics);
    let drawing = recorder.finish().unwrap();
    assert!(layout.width() > 0.0);
    assert!(!layout.glyph_runs().is_empty());
    assert!(
        drawing
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
    assert_eq!(descriptor.family(), "未安装字体 名称");
}
