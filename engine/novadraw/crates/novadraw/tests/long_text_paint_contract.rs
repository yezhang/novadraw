use novadraw::{
    figure::Figure,
    geometry::{Affine2D, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext, Path, TextPaintStatistics},
    render::{BackendCapabilities, CommandRecorder, RenderCommandKind, SurfaceInfo},
    runtime::Runtime,
    text::{
        BuiltinFont, FontDescriptor, TextConstraints, TextLayout,
        outline::{
            FontError, FontInstanceRef, GlyphOutline, GlyphOutlineProvider, OutlineTextEngine,
            SkrifaOutlineProvider,
        },
        shaping::{ParleyTextEngine, TextLayoutEngine},
    },
};
use std::sync::{Arc, Mutex};

#[test]
fn readonly_clip_tracks_nested_transform_restore_and_singular_fallback() {
    let mut recorder = CommandRecorder::new();
    let mut paint = recorder.paint();
    assert_eq!(paint.effective_clip_bounds(), None);
    paint
        .clip_rect(Rectangle::new(10.0, 20.0, 100.0, 80.0))
        .unwrap();
    paint.push_state();
    paint
        .concat_transform(Affine2D::from_translation(10.0, 20.0))
        .unwrap();
    assert_eq!(
        paint.effective_clip_bounds(),
        Some(Rectangle::new(0.0, 0.0, 100.0, 80.0))
    );
    paint
        .clip_rect(Rectangle::new(0.0, 5.0, 30.0, 40.0))
        .unwrap();
    assert_eq!(
        paint.effective_clip_bounds(),
        Some(Rectangle::new(0.0, 5.0, 30.0, 40.0))
    );
    paint.set_transform(Affine2D::from_scale(0.0, 1.0)).unwrap();
    assert_eq!(paint.effective_clip_bounds(), None);
    paint.pop_state().unwrap();
    assert_eq!(
        paint.effective_clip_bounds(),
        Some(Rectangle::new(10.0, 20.0, 100.0, 80.0))
    );
}

struct TextDrawing {
    layout: TextLayout,
    stroke: bool,
    work: Arc<Mutex<TextPaintStatistics>>,
}
impl Figure for TextDrawing {
    fn name(&self) -> &'static str {
        "ExternalLongTextDrawing"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 128.0, 128.0)
    }
    fn paint(&self, paint: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        paint.clip_rect(Rectangle::new(0.0, 0.0, 100.0, 40.0))?;
        paint.set_fill_paint(Color::BLACK);
        paint.set_stroke_paint(Color::BLACK);
        let result = if self.stroke {
            paint.stroke_text(&self.layout, Point::ZERO)
        } else {
            paint.fill_text(&self.layout, Point::ZERO)
        };
        *self.work.lock().unwrap() = paint.text_paint_statistics();
        result
    }
}

fn drawing(
    provider: Option<Box<dyn GlyphOutlineProvider>>,
    stroke: bool,
    certified: bool,
) -> (
    TextLayout,
    Vec<novadraw::render::RenderCommand>,
    TextPaintStatistics,
) {
    let mut runtime = Runtime::empty();
    let id = runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let mut engine: Box<dyn TextLayoutEngine> = match provider {
        Some(provider) => Box::new(OutlineTextEngine::new(
            Box::new(ParleyTextEngine::new()),
            provider,
        )),
        None => Box::new(ParleyTextEngine::new()),
    };
    engine
        .register_font(id.resource_id(), 1, BuiltinFont::Inter.bytes())
        .unwrap();
    let layout = engine
        .layout(
            &"AB CD EF GH\n".repeat(100),
            &FontDescriptor::new(BuiltinFont::Inter.family(), 16.0).unwrap(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let work = Arc::new(Mutex::new(TextPaintStatistics::default()));
    runtime
        .set_contents(Box::new(TextDrawing {
            layout: layout.clone(),
            stroke,
            work: work.clone(),
        }))
        .unwrap();
    let capabilities = BackendCapabilities::RETAINED_PARTIAL
        .with_path_clips()
        .with_custom_strokes();
    let capabilities = if certified {
        // A test-only certificate for these finite unhinted path inputs.
        capabilities.with_filled_path_coverage_outset(2)
    } else {
        capabilities
    };
    let frame = runtime
        .frames()
        .prepare_submission(
            SurfaceInfo {
                logical_width: 128.0,
                logical_height: 128.0,
                pixel_width: 128,
                pixel_height: 128,
                scale_factor: 1.0,
            },
            capabilities,
        )
        .into_ready()
        .unwrap();
    let work = *work.lock().unwrap();
    (layout, frame.commands, work)
}

fn path_operations(commands: &[novadraw::render::RenderCommand]) -> usize {
    commands
        .iter()
        .map(|command| match &command.kind {
            RenderCommandKind::FillPath { path, .. }
            | RenderCommandKind::StrokePath { path, .. } => path.operations().len(),
            _ => 0,
        })
        .sum()
}

#[test]
fn actual_outline_index_culls_only_with_coverage_certificate_and_keeps_stroke_fallback() {
    let (_, full, full_work) = drawing(Some(Box::new(SkrifaOutlineProvider)), false, false);
    let (_, clipped, clipped_work) = drawing(Some(Box::new(SkrifaOutlineProvider)), false, true);
    assert!(path_operations(&clipped) > 0);
    assert!(path_operations(&clipped) < path_operations(&full) / 10);
    assert!(clipped_work.indexed_nodes > 0);
    assert_eq!(clipped_work.candidate_glyphs, clipped_work.glyph_checks);
    assert_eq!(clipped_work.glyph_checks, clipped_work.selected_glyphs);
    assert_eq!(clipped_work.fallback_glyphs, 0);
    assert_eq!(full_work.selected_glyphs, full_work.fallback_glyphs);
    assert!(clipped_work.selected_glyphs < full_work.selected_glyphs / 10);
    let (_, stroke_full, _) = drawing(Some(Box::new(SkrifaOutlineProvider)), true, false);
    let (_, stroke_clipped, _) = drawing(Some(Box::new(SkrifaOutlineProvider)), true, true);
    assert_eq!(
        path_operations(&stroke_clipped),
        path_operations(&stroke_full)
    );
    let (_, native_full, _) = drawing(None, false, false);
    let (_, native_clipped, native_work) = drawing(None, false, true);
    let glyph_count = |commands: &[novadraw::render::RenderCommand]| -> usize {
        commands
            .iter()
            .map(|c| match &c.kind {
                RenderCommandKind::DrawGlyphRun { run, .. } => run.glyphs.len(),
                _ => 0,
            })
            .sum()
    };
    assert_eq!(glyph_count(&native_full), glyph_count(&native_clipped));
    assert!(glyph_count(&native_full) > 100);
    assert_eq!(native_work.indexed_nodes, 0);
    assert_eq!(
        native_work.fallback_glyphs as usize,
        glyph_count(&native_full)
    );
}

struct RemoteInk;
impl GlyphOutlineProvider for RemoteInk {
    fn revision(&self) -> u64 {
        1
    }
    fn outline(&mut self, _: FontInstanceRef<'_>, _: u32) -> Result<GlyphOutline, FontError> {
        let mut path = Path::new();
        path.move_to(-20_000.0, 0.0);
        path.line_to(1_000.0, 0.0);
        path.line_to(1_000.0, 60_000.0);
        path.line_to(-20_000.0, 60_000.0);
        path.close();
        GlyphOutline::new(path, 1000)
    }
}

#[test]
fn negative_bearing_and_ink_from_distant_lines_are_not_logically_pruned() {
    let (layout, commands, _) = drawing(Some(Box::new(RemoteInk)), false, true);
    let instances = layout.outlines().unwrap().instances();
    let visible = instances
        .iter()
        .filter(|instance| {
            let top_left = instance
                .glyph_to_layout()
                .transform_point(Point::new(-20_000.0, 60_000.0));
            let bottom_right = instance
                .glyph_to_layout()
                .transform_point(Point::new(1_000.0, 0.0));
            top_left.x() <= 102.0
                && bottom_right.x() >= -2.0
                && top_left.y() <= 42.0
                && bottom_right.y() >= -2.0
        })
        .count();
    let logical = layout
        .glyph_runs()
        .iter()
        .flat_map(|run| &run.glyphs)
        .filter(|glyph| glyph.y <= 42.0)
        .count();
    assert!(visible > logical * 5);
    assert_eq!(path_operations(&commands), visible * 5);
}

struct FlowDrawing {
    snapshot: Arc<novadraw::figure::flow::FlowSnapshot>,
    work: Arc<Mutex<TextPaintStatistics>>,
}
impl Figure for FlowDrawing {
    fn name(&self) -> &'static str {
        "ExternalFlowDrawing"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 128.0, 128.0)
    }
    fn paint(&self, paint: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        paint.clip_rect(Rectangle::new(0.0, 0.0, 100.0, 40.0))?;
        novadraw::figure::FigureDrawing::paint(&*self.snapshot, paint)?;
        *self.work.lock().unwrap() = paint.text_paint_statistics();
        Ok(())
    }
}

#[test]
fn paragraph_hierarchy_prunes_known_ink_and_keeps_unknown_native_paragraphs() {
    use novadraw::figure::flow::*;
    for native in [false, true] {
        let engine: Box<dyn TextLayoutEngine> = if native {
            Box::new(ParleyTextEngine::new())
        } else {
            Box::new(OutlineTextEngine::new(
                Box::new(ParleyTextEngine::new()),
                Box::new(SkrifaOutlineProvider),
            ))
        };
        let mut runtime =
            Runtime::with_text_layout_engine(novadraw::tree::FigureTree::new(), engine);
        runtime
            .resources()
            .register_builtin_font(BuiltinFont::Inter)
            .unwrap();
        let page = runtime
            .set_contents(Box::new(
                FlowPageFigure::new(Rectangle::new(0.0, 0.0, 100.0, 400.0))
                    .expect("valid detached figure input"),
            ))
            .unwrap();
        for _ in 0..128 {
            let block = runtime
                .figure(page)
                .unwrap()
                .add(Box::new(BlockFlowFigure::default()))
                .unwrap();
            runtime
                .figure(block)
                .unwrap()
                .add(Box::new(StructuredTextFigure::new("AB CD EF GH")))
                .unwrap();
        }
        runtime.frames().stabilize_for_query().unwrap();
        let snapshot: Arc<FlowSnapshot> = runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .shared_data()
            .unwrap();
        let work = Arc::new(Mutex::new(TextPaintStatistics::default()));
        runtime
            .set_contents(Box::new(FlowDrawing {
                snapshot,
                work: work.clone(),
            }))
            .unwrap();
        runtime
            .frames()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: 128.0,
                    logical_height: 128.0,
                    pixel_width: 128,
                    pixel_height: 128,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL.with_filled_path_coverage_outset(2),
            )
            .into_ready()
            .unwrap();
        let work = *work.lock().unwrap();
        if native {
            assert_eq!(work.candidate_paragraphs, 128);
            assert!(work.fallback_glyphs > 1000);
        } else {
            assert!(work.candidate_paragraphs > 0 && work.candidate_paragraphs < 10);
            assert!(work.paragraph_nodes < 32);
            assert_eq!(work.fallback_glyphs, 0);
        }
    }
}
