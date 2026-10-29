use super::gpu_test_support::GpuCanvas;
use novadraw::{
    figure::Figure,
    geometry::{Affine2D, Point, Rectangle},
    graphics::{Color, GraphicsError, PaintContext, Path},
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

struct RemoteInk;
impl GlyphOutlineProvider for RemoteInk {
    fn revision(&self) -> u64 {
        1
    }
    fn outline(&mut self, _: FontInstanceRef<'_>, _: u32) -> Result<GlyphOutline, FontError> {
        let mut path = Path::new();
        path.move_to(-1_000.0, 0.0);
        path.line_to(400.0, 0.0);
        path.line_to(400.0, 10_000.0);
        path.line_to(-1_000.0, 10_000.0);
        path.close();
        GlyphOutline::new(path, 1000)
    }
}

enum Drawing {
    Text(TextLayout),
    Flow(std::sync::Arc<novadraw::figure::flow::FlowSnapshot>),
}
impl Figure for Drawing {
    fn name(&self) -> &'static str {
        "LongTextGpuEvidence"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 128.0, 128.0)
    }
    fn paint(&self, paint: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        paint.concat_transform(Affine2D::new(0.9, 0.15, 0.12, 0.8, 10.0, 10.0))?;
        paint.clip_rect(Rectangle::new(0.0, 0.0, 100.25, 40.75))?;
        paint.push_state();
        paint.clip_rect(Rectangle::new(0.25, 0.25, 100.0, 40.5))?;
        paint.set_fill_paint(Color::BLACK);
        match self {
            Self::Text(layout) => paint.fill_text(layout, Point::new(0.125, 0.375))?,
            Self::Flow(snapshot) => novadraw::figure::FigureDrawing::paint(&**snapshot, paint)?,
        }
        paint.pop_state()
    }
}

fn fixture(remote: bool, flow: bool, live: bool) -> (Runtime, novadraw::tree::FigureId) {
    let mut runtime = Runtime::empty();
    let id = runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    let provider: Box<dyn GlyphOutlineProvider> = if remote {
        Box::new(RemoteInk)
    } else {
        Box::new(SkrifaOutlineProvider)
    };
    let mut engine = OutlineTextEngine::new(Box::new(ParleyTextEngine::new()), provider);
    if flow {
        use novadraw::figure::flow::*;
        let mut runtime =
            Runtime::with_text_layout_engine(novadraw::tree::FigureTree::new(), Box::new(engine));
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
        for _ in 0..50 {
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
        if live {
            return (runtime, page);
        }
        let snapshot = runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .shared_data::<FlowSnapshot>()
            .unwrap();
        let root = runtime
            .set_contents(Box::new(Drawing::Flow(snapshot)))
            .unwrap();
        return (runtime, root);
    }
    engine
        .register_font(id.resource_id(), 1, BuiltinFont::Inter.bytes())
        .unwrap();
    let layout = engine
        .layout(
            &"AB CD EF GH\n".repeat(50),
            &FontDescriptor::new(BuiltinFont::Inter.family(), 16.0).unwrap(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let root = runtime
        .set_contents(Box::new(Drawing::Text(layout)))
        .unwrap();
    (runtime, root)
}

#[test]
fn long_text_outline_full_partial_gpu_pixels() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/verification/long-text/pixels");
    for scale in [1.0, 1.5, 2.0] {
        for remote in [false, true] {
            for (flow, live) in [(false, false), (true, false), (true, true)] {
                let (mut full_runtime, _) = fixture(remote, flow, live);
                let (mut clipped_runtime, root) = fixture(remote, flow, live);
                let mut full = GpuCanvas::new(256, 256, scale);
                let mut clipped = GpuCanvas::new(256, 256, scale);
                let expected = full.render(&mut full_runtime, true);
                let actual =
                    clipped.render_with_coverage_certificate(&mut clipped_runtime, true, true);
                assert!(actual.chunks_exact(4).any(|pixel| pixel[3] != 0));
                assert_eq!(
                    actual, expected,
                    "full pixels: scale={scale}, remote={remote}"
                );
                clipped_runtime
                    .figure(root)
                    .unwrap()
                    .set_bounds(Rectangle::new(3.25, 4.75, 128.0, 128.0))
                    .unwrap();
                let partial =
                    clipped.render_with_coverage_certificate(&mut clipped_runtime, false, true);
                let rebuilt = clipped.render(&mut clipped_runtime, true);
                assert_eq!(
                    partial, rebuilt,
                    "partial pixels: scale={scale}, remote={remote}"
                );
                clipped.save(
                    output.join(format!(
                        "scale-{scale}-remote-{remote}-flow-{flow}-live-{live}.png"
                    )),
                    &partial,
                );
            }
        }
    }
}
