//! External consumers of the unified text/graphics protocol.
use novadraw::{
    Color, Dimension, Figure, FigureDrawing, FigureMeasurement, FigurePreparation,
    FigurePresentation, FigureTree, Point, Rectangle, Runtime,
    graphics::{Graphics, GraphicsError, PaintContext, Path},
    render::{
        BackendCapabilities, CommandRecorder, FontData, ParleyTextEngine, RenderCommandKind,
        SurfaceInfo, TextLayoutEngine,
    },
    text::{
        BuiltinFont, FontDescriptor, MeasureContext, TextConstraints, TextError, TextLayout,
        TextSystem,
        outline::{
            FontError, FontInstanceRef, GlyphGeometryKey, GlyphOutline, GlyphOutlineProvider,
            OutlineTextEngine, SkrifaOutlineProvider,
        },
    },
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

fn text_system() -> TextSystem {
    let mut text = TextSystem::new();
    text.register_builtin_font(BuiltinFont::Inter).unwrap();
    text
}

fn caption(
    gc: &mut PaintContext<'_>,
    text: &novadraw::text::TextLayout,
) -> Result<(), GraphicsError> {
    gc.set_fill_paint(Color::WHITE);
    gc.fill_rect(Rectangle::new(
        0.0,
        0.0,
        text.size().width,
        text.size().height,
    ))?;
    gc.set_fill_paint(Color::BLACK);
    gc.fill_text(text, Point::new(0.0, 0.0))
}

#[test]
fn one_context_measures_and_records_a_retained_layout() {
    let mut text = text_system();
    let mut recorder = CommandRecorder::new();
    let mut gc = Graphics::new(&mut text, &mut recorder);
    let layout = gc
        .layout_text("office e\u{301}", TextConstraints::UNBOUNDED)
        .unwrap();
    assert_eq!(
        gc.measure_text("office e\u{301}", TextConstraints::UNBOUNDED)
            .unwrap(),
        layout.metrics()
    );
    let font = gc.font_metrics().unwrap();
    assert!(font.ascent() > 0.0 && font.descent() > 0.0);
    assert_eq!(font.face(), &layout.glyph_runs()[0].font);
    caption(&mut gc.paint(), &layout).unwrap();
    drop(gc);
    drop(text);
    let drawing = recorder.finish().unwrap();
    assert!(matches!(
        drawing.commands()[0].kind,
        RenderCommandKind::FillRect { .. }
    ));
    assert!(
        drawing
            .commands()
            .iter()
            .any(|c| matches!(c.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
    assert_eq!(drawing.resources().ready.len(), 1);
}

#[test]
fn stale_and_foreign_layouts_fail_without_partial_recording() {
    let mut text = text_system();
    let layout = text
        .measure(FontDescriptor::default())
        .layout_text("AB", TextConstraints::UNBOUNDED)
        .unwrap();
    let mut other = text_system();
    let mut recorder = CommandRecorder::new();
    let mut gc = Graphics::new(&mut other, &mut recorder);
    gc.set_fill_paint(Color::BLACK);
    gc.fill_rect(Rectangle::new(0.0, 0.0, 10.0, 10.0)).unwrap();
    assert!(gc.fill_text(&layout, Point::new(0.0, 0.0)).is_err());
    drop(gc);
    assert_eq!(recorder.commands().len(), 1);
    assert!(
        recorder.finish().is_err(),
        "ignored recording failures must block publication"
    );

    let mut text = TextSystem::new();
    let id = text.register_builtin_font(BuiltinFont::Inter).unwrap();
    let layout = text
        .measure(FontDescriptor::default())
        .layout_text("old", TextConstraints::UNBOUNDED)
        .unwrap();
    text.replace_font(id, FontData::new(BuiltinFont::Inter.bytes().to_vec()))
        .unwrap();
    let mut recorder = CommandRecorder::new();
    let mut gc = Graphics::new(&mut text, &mut recorder);
    gc.set_fill_paint(Color::BLACK);
    assert!(matches!(
        gc.fill_text(&layout, Point::new(0.0, 0.0)),
        Err(GraphicsError::Font(FontError::ResourceMismatch))
    ));
    drop(gc);
    assert!(recorder.commands().is_empty());
}

#[test]
fn recording_rejects_two_revisions_of_the_same_resource() {
    let mut text = TextSystem::new();
    let id = text.register_builtin_font(BuiltinFont::Inter).unwrap();
    let mut recorder = CommandRecorder::new();
    {
        let mut gc = Graphics::new(&mut text, &mut recorder);
        let layout = gc.layout_text("first", TextConstraints::UNBOUNDED).unwrap();
        gc.set_fill_paint(Color::BLACK);
        gc.fill_text(&layout, Point::new(0.0, 0.0)).unwrap();
    }
    let before = recorder.commands().len();
    text.replace_font(id, FontData::new(BuiltinFont::Inter.bytes().to_vec()))
        .unwrap();
    {
        let mut gc = Graphics::new(&mut text, &mut recorder);
        let layout = gc
            .layout_text("second", TextConstraints::UNBOUNDED)
            .unwrap();
        assert!(matches!(
            gc.fill_text(&layout, Point::new(0.0, 0.0)),
            Err(GraphicsError::ConflictingResource(_))
        ));
    }
    assert_eq!(recorder.commands().len(), before);
    assert!(recorder.finish().is_err());
}

#[test]
fn font_state_restores_and_does_not_reinterpret_prepared_text() {
    let mut text = text_system();
    let mut recorder = CommandRecorder::new();
    let mut gc = Graphics::new(&mut text, &mut recorder);
    let layout = gc
        .layout_text("original", TextConstraints::UNBOUNDED)
        .unwrap();
    let size = layout.size();
    let original = gc.font().clone();
    gc.push_state();
    gc.set_font(FontDescriptor::new(BuiltinFont::Inter.family(), 40.0).unwrap());
    assert!(
        gc.measure_text("original", TextConstraints::UNBOUNDED)
            .unwrap()
            .size()
            .width
            > size.width
    );
    gc.restore_state();
    assert_eq!(gc.font(), &original);
    gc.pop_state();
    gc.set_fill_paint(Color::BLACK);
    gc.fill_text(&layout, Point::new(0.0, 0.0)).unwrap();
    assert_eq!(layout.size(), size);
    drop(gc);
    recorder.finish().unwrap();
}

#[test]
fn independent_paths_can_be_filled_and_stroked_repeatedly() {
    let mut text = text_system();
    let mut recorder = CommandRecorder::new();
    let mut gc = Graphics::new(&mut text, &mut recorder);
    let mut path = Path::new();
    path.move_to(0.0, 0.0);
    path.quad_to(10.0, 20.0, 30.0, 0.0);
    path.close();
    gc.set_fill_paint(Color::RED);
    gc.set_stroke_paint(Color::BLACK);
    gc.fill_path(&path).unwrap();
    gc.stroke_path(&path).unwrap();
    gc.fill_path(&path).unwrap();
    drop(gc);
    assert_eq!(recorder.finish().unwrap().commands().len(), 3);
    assert_eq!(path.operations().len(), 3);
}

#[test]
fn scoped_paint_helpers_cannot_consume_the_callers_state() {
    let mut text = text_system();
    let mut recorder = CommandRecorder::new();
    let mut graphics = Graphics::new(&mut text, &mut recorder);
    graphics.push_state();
    {
        let mut paint = graphics.paint();
        paint.pop_state();
    }
    graphics.pop_state();
    drop(graphics);
    assert!(matches!(
        recorder.finish(),
        Err(GraphicsError::UnbalancedState)
    ));

    let mut recorder = CommandRecorder::new();
    let mut graphics = Graphics::new(&mut text, &mut recorder);
    assert!(graphics.set_stroke_width(-1.0).is_err());
    drop(graphics);
    assert!(recorder.commands().is_empty());
    assert!(recorder.finish().is_err());
}

#[test]
fn outlines_preserve_beziers_and_distinguish_space_from_missing_glyph() {
    let mut text = text_system();
    let layout = text
        .measure(FontDescriptor::default())
        .layout_text("O ", TextConstraints::UNBOUNDED)
        .unwrap();
    let run = &layout.glyph_runs()[0];
    let resource = text
        .resources()
        .snapshot_resource(run.font.resource_id())
        .unwrap();
    let instance = FontInstanceRef::new(&run.font, &resource, &run.normalized_coords).unwrap();
    let mut provider = SkrifaOutlineProvider;
    let outline = provider.outline(instance, run.glyphs[0].id).unwrap();
    assert!(outline.units_per_em() > 0);
    assert!(outline.ink_bounds().is_some());
    assert!(
        outline
            .path()
            .operations()
            .iter()
            .any(|op| matches!(op, novadraw::graphics::PathOp::QuadTo(..)))
    );
    let space = provider
        .outline(instance, run.glyphs.last().unwrap().id)
        .unwrap();
    assert!(space.path().operations().is_empty());
    assert!(matches!(
        provider.outline(instance, u32::MAX),
        Err(FontError::MissingGlyph(_))
    ));
    assert!(layout.size().width > 0.0);
}

struct CountingProvider {
    calls: Arc<AtomicUsize>,
    revision: Arc<AtomicU64>,
}

impl GlyphOutlineProvider for CountingProvider {
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    fn outline(
        &mut self,
        _font: FontInstanceRef<'_>,
        glyph: u32,
    ) -> Result<GlyphOutline, FontError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let side = 400.0 + f64::from(glyph % 5);
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(side, 0.0);
        path.line_to(side, side);
        path.close();
        GlyphOutline::new(path, 1_000)
    }
}

#[test]
fn custom_outline_assets_are_cached_separately_from_glyph_instances() {
    let calls = Arc::new(AtomicUsize::new(0));
    let revision = Arc::new(AtomicU64::new(1));
    let provider = CountingProvider {
        calls: calls.clone(),
        revision: revision.clone(),
    };
    let mut engine = OutlineTextEngine::new(Box::new(ParleyTextEngine::new()), Box::new(provider));
    let font_id = novadraw::render::ResourceId::new(uuid::Uuid::nil(), 17);
    engine
        .register_font(font_id, 1, BuiltinFont::Inter.bytes())
        .unwrap();

    let first = engine
        .layout(
            "A A",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let outlined = first.outlines().expect("outline adapter selected");
    let mut prepared = HashMap::<GlyphGeometryKey, usize>::new();
    for instance in outlined.instances() {
        prepared
            .entry(instance.key().clone())
            .or_insert_with(|| instance.outline().path().operations().len());
    }
    assert_eq!(prepared.len(), calls.load(Ordering::Relaxed));
    assert!(outlined.instances().len() > prepared.len());
    let calls_after_first = calls.load(Ordering::Relaxed);

    let second = engine
        .layout(
            "A A",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let second_interaction_revision = second
        .hit_test_text(Point::new(0.0, 0.0))
        .unwrap()
        .layout_revision();
    assert_eq!(calls.load(Ordering::Relaxed), calls_after_first);
    assert_eq!(first.metrics(), second.metrics());

    revision.store(2, Ordering::Relaxed);
    let third = engine
        .layout(
            "A A",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    assert_ne!(
        second.key().engine_revision(),
        third.key().engine_revision()
    );
    assert_ne!(
        second_interaction_revision,
        third
            .hit_test_text(Point::new(0.0, 0.0))
            .unwrap()
            .layout_revision()
    );
    assert!(calls.load(Ordering::Relaxed) > calls_after_first);
    assert_eq!(second.metrics(), third.metrics());

    let mut text = TextSystem::with_engine(Box::new(OutlineTextEngine::new(
        Box::new(ParleyTextEngine::new()),
        Box::new(CountingProvider {
            calls: Arc::new(AtomicUsize::new(0)),
            revision: Arc::new(AtomicU64::new(1)),
        }),
    )));
    text.register_builtin_font(BuiltinFont::Inter).unwrap();
    let mut recorder = CommandRecorder::new();
    let mut graphics = Graphics::new(&mut text, &mut recorder);
    let layout = graphics
        .layout_text("path consumer", TextConstraints::UNBOUNDED)
        .unwrap();
    graphics.set_fill_paint(Color::BLACK);
    graphics.fill_text(&layout, Point::new(8.0, 12.0)).unwrap();
    drop(graphics);
    let drawing = recorder.finish().unwrap();
    assert!(
        drawing
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::FillPath { .. }))
    );
    assert!(
        !drawing
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
    assert_eq!(drawing.resources().ready.len(), 1);
}

#[test]
fn outlined_fallback_rtl_and_variations_preserve_layout_metrics() {
    fn register_fonts(text: &mut TextSystem) {
        for font in [
            BuiltinFont::Inter,
            BuiltinFont::NotoSansSc,
            BuiltinFont::NotoSansArabic,
        ] {
            text.register_builtin_font(font).unwrap();
        }
    }

    let mut native = TextSystem::new();
    register_fonts(&mut native);
    let mut outlined = TextSystem::with_engine(Box::new(OutlineTextEngine::new(
        Box::new(ParleyTextEngine::new()),
        Box::new(SkrifaOutlineProvider),
    )));
    register_fonts(&mut outlined);
    let font = FontDescriptor::new(BuiltinFont::Inter.family(), 24.0)
        .unwrap()
        .with_weight(700.0)
        .unwrap();
    let source = "office 中文 العربية";
    let native = native
        .measure(font.clone())
        .layout_text(source, TextConstraints::UNBOUNDED)
        .unwrap();
    let outlined = outlined
        .measure(font)
        .layout_text(source, TextConstraints::UNBOUNDED)
        .unwrap();

    assert_eq!(native.metrics(), outlined.metrics());
    assert!(outlined.glyph_runs().len() >= 3);
    assert!(
        outlined
            .glyph_runs()
            .iter()
            .any(|run| !run.normalized_coords.is_empty())
    );
    assert_eq!(
        outlined.outlines().unwrap().instances().len(),
        outlined
            .glyph_runs()
            .iter()
            .map(|run| run.glyphs.len())
            .sum::<usize>()
    );
}

struct PreparedCaption {
    text: String,
}

struct CaptionDrawing {
    layout: TextLayout,
}

impl FigureDrawing for CaptionDrawing {
    fn paint(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        context.set_fill_paint(Color::BLACK);
        context.fill_text(&self.layout, Point::new(0.0, 0.0))
    }
}

impl FigurePreparation for PreparedCaption {
    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        _bounds: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        let layout = context.layout_text(&self.text, TextConstraints::UNBOUNDED)?;
        let metrics = layout.metrics();
        let size = metrics.size();
        FigurePresentation::new(
            FigureMeasurement::new(size.width, size.height, Some(f64::from(metrics.baseline()))),
            Dimension::new(0.0, size.height),
            Rectangle::new(0.0, 0.0, size.width, size.height),
            Arc::new(CaptionDrawing { layout }),
        )
    }
}

impl Figure for PreparedCaption {
    fn name(&self) -> &'static str {
        "PreparedCaption"
    }

    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 160.0, 40.0)
    }

    fn preparation(&self) -> Option<&dyn FigurePreparation> {
        Some(self)
    }
}

#[test]
fn external_figure_prepares_and_paints_through_phase_limited_contexts() {
    let mut tree = FigureTree::new();
    let figure = tree.builder().set_contents(Box::new(PreparedCaption {
        text: "prepared figure".to_owned(),
    }));
    let mut runtime = Runtime::new(tree);
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let submission = runtime
        .prepare_submission(
            SurfaceInfo {
                logical_width: 200.0,
                logical_height: 80.0,
                pixel_width: 200,
                pixel_height: 80,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .unwrap();
    assert!(
        runtime
            .tree()
            .preferred_measurement(figure, novadraw::MeasureConstraints::UNBOUNDED)
            .unwrap()
            .baseline
            .is_some()
    );
    assert!(
        submission
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
}
